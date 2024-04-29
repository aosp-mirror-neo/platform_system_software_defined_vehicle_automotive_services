// Copyright 2023 Google LLC

use itertools::Itertools;
use std::collections::HashSet;

use sdvgenerated::sdv_user_preferences_user_preferences_admin_service_interface::UserPreferencesAdminService;
use sdvgenerated::sdv_user_preferences_user_preferences_management_service_interface::UserPreferencesManagementService;
use sdvgenerated::sdv_user_preferences_user_preferences_registry_service_interface::UserPreferencesRegistryService;
use sdvgenerated_rpc_sdv_user_preferences_user_controllable_user_controllable_service::client::UserControllableServiceClient;
use sdvgenerated_rpc_sdv_user_preferences_user_controllable_user_controllable_service::interface::UserControllableService;
use sdvgenerated_rpc_sdv_user_preferences_user_preferences_view::interface::UserPreferencesView;

use sdvgenerated_rpc_sdv_user_preferences_user_preferences_view::client::UserPreferencesViewClient;
use sdvmiddleware_rpc_grpc::grpc_client_transport::GrpcClientTransport;

use user_preferences_api::settings_group::SettingsGroupId as SettingsGroupIdProto;
use user_preferences_api::user::{user::UserFlags, User};
use user_preferences_api::user_preferences_registry_service::{
    RegisterSettingsRequest, RegisterSettingsResponse,
};

use user_preferences_api::error::{Error, ResponseCode};
use user_preferences_api::setting::{Setting as SettingProto, SettingKind};
use user_preferences_api::user_preferences_admin_service::*;
use user_preferences_api::user_preferences_management_service::*;
use user_preferences_api::user_preferences_registry_service::{
    UpdateSettingsRequest, UpdateSettingsResponse,
};

use protobuf::{Enum, EnumOrUnknown, MessageField};
use sdvmiddleware_rpc::RpcResult;
use std::collections::HashMap;
use std::sync::Mutex;
use std::vec::Vec;
use user_preferences_api::user_preferences_view::OnSettingsChangeRequest;

use user_preferences_api::user_controllable_service;

use crate::macros::{acquire_lock_or_return_err, error_response};
use crate::setting::Setting;
use crate::settings_group::SettingsGroup;
use crate::settings_group_id::SettingsGroupId;
use crate::vehicle_configuration::VehicleConfiguration;
use user_preferences_utils::{print_setting_definition, print_settings};

type Users = HashMap<i32, VehicleConfiguration>;
pub struct UserPreferencesServiceImpl {
    vehicle_state: Mutex<VehicleConfiguration>,
    users: Mutex<Users>,
    current_user_id: Mutex<Option<i32>>,
    change_subscribers: Mutex<HashMap<SettingsGroupId, HashSet<String>>>,
}

impl UserPreferencesServiceImpl {
    pub fn new() -> Self {
        UserPreferencesServiceImpl {
            vehicle_state: Mutex::new(VehicleConfiguration::new(User::new())),
            users: Mutex::new(Users::new()),
            current_user_id: Mutex::new(None),
            change_subscribers: Mutex::new(HashMap::new()),
        }
    }

    fn apply_settings(
        settings_group_id: SettingsGroupIdProto,
        settings: Vec<SettingProto>,
    ) -> Result<(), String> {
        // Ideally the client should be created via the get_user_controllable_service_client method
        // that is present in the UserPreferencesServer, for now manually create it since the rust
        // compiler cannot determine the correctness of the program if UserPreferencesServiceImpl & UserPreferencesServer
        // have references to each other
        let client: UserControllableServiceClient = UserControllableServiceClient::new(Box::new(
            GrpcClientTransport::new(&settings_group_id.service_fqin),
        ));

        let resp = client
            .request_settings_change(user_controllable_service::RequestSettingsChangeRequest {
                group_name: settings_group_id.name,
                settings,
                ..Default::default()
            })
            .map_err(|x| {
                format!("Failed to perform GRPC request to request_settings_change: {}", x)
            })?;

        match resp.error.is_some() {
            true => Err(format!("Failed to request settings changes: {}", resp.error.unwrap())),
            false => Ok(()),
        }
    }

    fn reset_services() -> Result<(), String> {
        let client =
            UserControllableServiceClient::new(Box::new(GrpcClientTransport::new("HvacService")));

        let resp =
            client.factory_reset(user_controllable_service::FactoryResetRequest::new()).map_err(
                |x| format!("Failed to perform GRPC request to request_settings_change: {}", x),
            )?;

        match resp.error.is_some() {
            true => {
                Err(format!("Failed to factory reset HvacService service: {}", resp.error.unwrap()))
            }
            false => Ok(()),
        }
    }

    fn are_valid_user_flags(user_flags: i32) -> bool {
        // Ensure driver flag is set since only driver is supported now
        (user_flags & UserFlags::DRIVER.value()) == UserFlags::DRIVER.value() &&
        // Ensure that no other bits are set other than ephemeral & driver
        user_flags <=  (UserFlags::EPHEMERAL.value() | UserFlags::DRIVER.value())
    }

    fn notify_subscribers(
        settings_group_id: &SettingsGroupId,
        version: &String,
        setting: &[SettingProto],
        subscribers: &[String],
    ) {
        // Ideally the client should be created via the get_user_controllable_service_client method
        // that is present in the UserPreferencesServer, for now manually create it since the rust
        // compiler cannot determine the correctness of the program if UserPreferencesServiceImpl & UserPreferencesServer
        // have references to each other
        for subscriber in subscribers {
            let client =
                UserPreferencesViewClient::new(Box::new(GrpcClientTransport::new(subscriber)));

            println!("Notifying {} of settings change\n", subscriber);
            client
                .on_settings_change(OnSettingsChangeRequest {
                    settings_group_id: MessageField::some(settings_group_id.clone().into()),
                    version: version.to_string(),
                    settings: setting.to_vec(),
                    //TODO: Expose pending and persisted settings for the user to subscribers
                    ..Default::default()
                })
                .expect("Failed to notify clients of settings changes");
        }
    }

    fn validate_requested_setting_changes(
        &self,
        current_settings: &SettingsGroup,
        requested_setting_changes: &[SettingProto],
    ) -> Result<(), String> {
        for setting in requested_setting_changes.iter() {
            let current_settings = match current_settings.settings.get(&setting.key) {
                Some(x) => x,
                None => {
                    return Err(format!(
                        "Setting {} cannot be found within setting group {}",
                        setting.key, current_settings.id
                    ))
                }
            };
            if let Some(value) = &setting.value {
                if !current_settings.is_valid_value(value) {
                    return Err(format!(
                        "{:#?} is not a valid value for Setting {}",
                        value, setting.key
                    ));
                }
            } else {
                return Err(format!("Setting {} has None value", setting.key));
            }
        }

        Ok(())
    }
}

impl UserPreferencesAdminService for UserPreferencesServiceImpl {
    fn create_user(&self, request: CreateUserRequest) -> RpcResult<CreateUserResponse> {
        if request.user.is_none() {
            println!("Invalid create user request, user field cannot be empty");
            return error_response!(
                CreateUserResponse,
                ResponseCode::INVALID_REQUEST,
                "Invalid create user request, user field cannot be empty",
            );
        }
        let user = request.user.unwrap();

        let mut users = acquire_lock_or_return_err!(self.users, CreateUserResponse);
        if users.contains_key(&user.id) {
            println!("Invalid create user request, ID {} is already in use", user.id);
            return error_response!(
                CreateUserResponse,
                ResponseCode::ALREADY_EXISTS,
                "User ID '{}' is already in use",
                user.id
            );
        }
        if !Self::are_valid_user_flags(user.flags) {
            println!("Invalid user flags set");
            return error_response!(
                CreateUserResponse,
                ResponseCode::INVALID_REQUEST,
                "User flags {} is not a valid value",
                user.flags
            );
        }

        // Initialize user with default values
        let vehicle_state = acquire_lock_or_return_err!(self.vehicle_state, CreateUserResponse);
        let mut new_user = vehicle_state.clone();
        new_user.reset_settings();
        new_user.user = User { id: user.id, flags: user.flags, ..Default::default() };

        println!("Created user {}", user.id);

        users.insert(user.id, new_user);

        Ok(CreateUserResponse::new())
    }

    fn select_user(&self, request: SelectUserRequest) -> RpcResult<SelectUserResponse> {
        let mut users = acquire_lock_or_return_err!(self.users, SelectUserResponse);
        let mut current_user_id =
            acquire_lock_or_return_err!(self.current_user_id, SelectUserResponse);

        let target_user = match users.get(&request.user_id) {
            Some(x) => x,
            None => {
                return error_response!(
                    SelectUserResponse,
                    ResponseCode::NOT_FOUND_ERROR,
                    "User ID '{}' does not exist",
                    request.user_id
                )
            }
        };

        println!("Loading user {}\n", request.user_id);

        for (settings_group_id, settings_group) in target_user.settings_groups.iter() {
            let settings = settings_group
                .settings
                .values()
                .filter(|x| x.kind.as_ref() == &SettingKind::PER_USER)
                .cloned()
                .map(SettingProto::from)
                .collect();

            if let Err(result) = Self::apply_settings(settings_group_id.clone().into(), settings) {
                println!("Failed to apply settings changes: {}", result);
            }
        }

        // If the previous user is ephemeral, then delete it
        if let Some(id) =
            current_user_id.filter(|&id| users.get(&id).map_or(false, |p| p.is_ephemeral()))
        {
            users.remove(&id);
            println!("Deleted ephemeral user: {}", id);
        }
        *current_user_id = Some(request.user_id);

        Ok(SelectUserResponse::new())
    }

    fn delete_user(&self, request: DeleteUserRequest) -> RpcResult<DeleteUserResponse> {
        let mut users = acquire_lock_or_return_err!(self.users, DeleteUserResponse);
        let current_user_id = acquire_lock_or_return_err!(self.current_user_id, DeleteUserResponse);

        if !users.contains_key(&request.user_id) {
            return Ok(DeleteUserResponse {
                error: MessageField::some(Error {
                    error_code: EnumOrUnknown::new(ResponseCode::NOT_FOUND_ERROR),
                    message: format!("User {} does not exist", request.user_id),
                    ..Default::default()
                }),
                ..Default::default()
            });
        }
        if current_user_id.is_some_and(|id| id == request.user_id) {
            return Ok(DeleteUserResponse {
                error: MessageField::some(Error {
                    error_code: EnumOrUnknown::new(ResponseCode::INVALID_REQUEST),
                    message: format!("Cannot delete User {} since it is in use", request.user_id),
                    ..Default::default()
                }),
                ..Default::default()
            });
        }

        users.remove(&request.user_id);
        println!("Deleted user {}", &request.user_id);

        Ok(DeleteUserResponse::new())
    }

    fn factory_reset(&self, _request: FactoryResetRequest) -> RpcResult<FactoryResetResponse> {
        let mut users = acquire_lock_or_return_err!(self.users, FactoryResetResponse);
        users.clear();

        let mut current_user_id =
            acquire_lock_or_return_err!(self.current_user_id, FactoryResetResponse);
        *current_user_id = None;

        let resp = match Self::reset_services() {
            Ok(_) => FactoryResetResponse::new(),
            Err(err) => FactoryResetResponse {
                error: MessageField::some(Error {
                    error_code: EnumOrUnknown::new(ResponseCode::INTERNAL_ERROR),
                    message: err,
                    ..Default::default()
                }),
                ..Default::default()
            },
        };

        Ok(resp)
    }

    fn list_users(&self, _request: ListUsersRequest) -> RpcResult<ListUsersResponse> {
        let users = acquire_lock_or_return_err!(self.users, ListUsersResponse);
        let users = users.iter().map(|(_, config)| config.user.clone()).collect_vec();

        Ok(ListUsersResponse { users, ..Default::default() })
    }

    fn get_user_settings(
        &self,
        request: GetUserSettingsRequest,
    ) -> RpcResult<GetUserSettingsResponse> {
        let users = acquire_lock_or_return_err!(self.users, GetUserSettingsResponse);

        let user = match users.get(&request.user_id) {
            Some(user) => user,
            None => {
                return error_response!(
                    GetUserSettingsResponse,
                    ResponseCode::INVALID_REQUEST,
                    "user {} does not exist",
                    request.user_id
                )
            }
        };

        return Ok(GetUserSettingsResponse {
            groups: user.settings_groups.values().cloned().map_into().collect(),
            ..Default::default()
        });
    }
}

impl UserPreferencesManagementService for UserPreferencesServiceImpl {
    fn request_settings_change(
        &self,
        request: RequestSettingsChangeRequest,
    ) -> RpcResult<RequestSettingsChangeResponse> {
        let vehicle_state =
            acquire_lock_or_return_err!(self.vehicle_state, RequestSettingsChangeResponse);

        let settings_group_id: SettingsGroupId = request.settings_group_id.unwrap().into();

        let current_settings = match vehicle_state.settings_groups.get(&settings_group_id) {
            Some(settings) => settings,
            None => {
                return error_response!(
                    RequestSettingsChangeResponse,
                    ResponseCode::INVALID_REQUEST,
                    "Settings group {} not found",
                    settings_group_id
                )
            }
        };

        if let Err(err) =
            self.validate_requested_setting_changes(current_settings, &request.settings)
        {
            return error_response!(
                RequestSettingsChangeResponse,
                ResponseCode::INVALID_REQUEST,
                "{}",
                err,
            );
        }

        println!("Updating the following settings within {}:", settings_group_id);
        print_settings(&request.settings);

        let resp = match Self::apply_settings(settings_group_id.clone().into(), request.settings) {
            Ok(_) => RequestSettingsChangeResponse::new(),
            Err(err) => RequestSettingsChangeResponse {
                error: MessageField::some(Error {
                    error_code: EnumOrUnknown::new(ResponseCode::RUNTIME_ERROR),
                    message: err,
                    ..Default::default()
                }),
                ..Default::default()
            },
        };
        Ok(resp)
    }

    fn subscribe_to_settings_change_and_get_settings(
        &self,
        request: SubscribeToSettingsChangeAndGetSettingsRequest,
    ) -> RpcResult<SubscribeToSettingsChangeAndGetSettingsResponse> {
        let vehicle_state = acquire_lock_or_return_err!(
            self.vehicle_state,
            SubscribeToSettingsChangeAndGetSettingsResponse
        );
        let mut change_subscribers = acquire_lock_or_return_err!(
            self.change_subscribers,
            SubscribeToSettingsChangeAndGetSettingsResponse
        );
        let settings_group_id: SettingsGroupId = request.settings_group_id.unwrap().into();

        change_subscribers
            .entry(settings_group_id.clone())
            .or_default()
            .insert(request.caller_fqin);

        if let Some(settings_group) = vehicle_state.settings_groups.get(&settings_group_id) {
            let result = SubscribeToSettingsChangeAndGetSettingsResponse {
                setting_and_constraints: settings_group
                    .settings
                    .values()
                    .cloned()
                    .map(|setting| setting.into())
                    .collect(),
                version: settings_group.version.clone(),
                ..Default::default()
            };

            println!("Successfully registered subscriber to settings: {}\n", settings_group_id);
            return Ok(result);
        }

        println!("Received request to subscribe to non existent settings: {}", settings_group_id);

        error_response!(
            SubscribeToSettingsChangeAndGetSettingsResponse,
            ResponseCode::INVALID_REQUEST,
            "Settings {} does not exist",
            settings_group_id
        )
    }

    fn unsubscribe_from_settings_change(
        &self,
        request: UnsubscribeToSettingsChangeRequest,
    ) -> RpcResult<UnsubscribeToSettingsChangeResponse> {
        let mut change_subscribers = acquire_lock_or_return_err!(
            self.change_subscribers,
            UnsubscribeToSettingsChangeResponse
        );

        let settings_group_id: SettingsGroupId = request.settings_group_id.unwrap().into();

        if let Some(settings_group_subscribers) = change_subscribers.get_mut(&settings_group_id) {
            settings_group_subscribers.remove(&request.caller_fqin);
        } else {
            return error_response!(
                UnsubscribeToSettingsChangeResponse,
                ResponseCode::INVALID_REQUEST,
                "Settings group {} does not exist",
                settings_group_id
            );
        }

        Ok(UnsubscribeToSettingsChangeResponse::new())
    }
}

impl UserPreferencesRegistryService for UserPreferencesServiceImpl {
    fn register_settings(
        &self,
        request: RegisterSettingsRequest,
    ) -> RpcResult<RegisterSettingsResponse> {
        let mut vehicle_state =
            acquire_lock_or_return_err!(self.vehicle_state, RegisterSettingsResponse);

        let settings_group_id =
            SettingsGroupId::new(request.caller_fqin.clone(), request.group_name.clone());

        if !vehicle_state.settings_groups.contains_key(&settings_group_id)
            || vehicle_state.settings_groups.get(&settings_group_id).unwrap().version
                != request.version
        {
            println!("Registered settings {}", settings_group_id);

            print_setting_definition(&request.settings_definitions);

            // Also register the settings in the vehicle state
            let settings_group =
                vehicle_state.settings_groups.entry(settings_group_id.clone()).or_insert(
                    SettingsGroup::new(settings_group_id.clone(), request.version.clone(), vec![]),
                );

            request.settings_definitions.iter().cloned().for_each(|setting_definition| {
                let setting = Setting::from(setting_definition);

                if !settings_group.settings.contains_key(&setting.name) {
                    settings_group.settings.insert(setting.name.clone(), setting);
                }
            });
        } else {
            println!(
                "Settings group {} with version {} is already registered",
                settings_group_id, request.version
            );
        }

        Ok(RegisterSettingsResponse::new())
    }

    fn update_settings(&self, request: UpdateSettingsRequest) -> RpcResult<UpdateSettingsResponse> {
        let mut vehicle_state =
            acquire_lock_or_return_err!(self.vehicle_state, UpdateSettingsResponse);
        let change_subscribers =
            acquire_lock_or_return_err!(self.change_subscribers, UpdateSettingsResponse);

        let settings_group_id = SettingsGroupId::new(request.caller_fqin, request.group_name);

        let service_settings_group = match vehicle_state.settings_groups.get_mut(&settings_group_id)
        {
            Some(x) => x,
            None => {
                return error_response!(
                    UpdateSettingsResponse,
                    ResponseCode::NOT_FOUND_ERROR,
                    "Settings group {} not found",
                    settings_group_id
                )
            }
        };

        if request.settings.iter().any(|setting| setting.value.is_none()) {
            return error_response!(
                UpdateSettingsResponse,
                ResponseCode::INVALID_REQUEST,
                "Encountered settings with a None value: {:?}",
                request.settings
            );
        }

        request.settings.iter().cloned().for_each(|x| {
            if let Some(setting) = service_settings_group.settings.get_mut(&x.key) {
                if let Err(err) = setting.set_value(x.value.unwrap()) {
                    println!("Cannot update setting {}: ", err);
                }
            }
        });

        println!("Updated the following settings");
        print_settings(&request.settings);

        let current_user_id =
            acquire_lock_or_return_err!(self.current_user_id, UpdateSettingsResponse);

        if let Some(current_user_id) = *current_user_id {
            let mut users = acquire_lock_or_return_err!(self.users, UpdateSettingsResponse);
            let current_user = users.get_mut(&current_user_id).unwrap();
            let current_settings_group =
                current_user.settings_groups.get_mut(&settings_group_id).unwrap();

            request.settings.iter().cloned().for_each(|x| {
                if let Some(setting) = current_settings_group.settings.get_mut(&x.key) {
                    if let Err(err) = setting.set_value(x.value.unwrap()) {
                        println!("Cannot update setting {}: ", err);
                    }
                }
            });
        }

        Self::notify_subscribers(
            &settings_group_id,
            &service_settings_group.version,
            &request.settings,
            &change_subscribers
                .get(&settings_group_id)
                .unwrap_or(&HashSet::new())
                .iter()
                .cloned()
                .collect_vec(),
        );

        Ok(UpdateSettingsResponse::new())
    }
}
