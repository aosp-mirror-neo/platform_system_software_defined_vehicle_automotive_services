// Copyright 2023 Google LLC

use itertools::Itertools;
use std::collections::HashSet;

use sdvgenerated::user_profile_admin_service_interface::UserProfileAdminService;
use sdvgenerated::user_profile_management_service_interface::UserProfileManagementService;
use sdvgenerated::user_profile_registry_service_interface::UserProfileRegistryService;
use sdvgenerated_rpc_user_controllable_service::client::UserControllableServiceClient;
use sdvgenerated_rpc_user_controllable_service::interface::UserControllableService;
use sdvgenerated_rpc_user_profile_view::interface::UserProfileView;

use sdvgenerated_rpc_user_profile_view::client::UserProfileViewClient;
use sdvmiddleware_rpc_grpc::grpc_client_transport::GrpcClientTransport;

use user_profile_api::settings_group::SettingsGroup as SettingsGroupProto;
use user_profile_api::user::User;
use user_profile_api::user_profile_registry_service::{
    RegisterSettingsRequest, RegisterSettingsResponse,
};

use user_profile_api::error::{Error, ResponseCode};
use user_profile_api::setting::{Setting as SettingProto, SettingKind};
use user_profile_api::user_profile_admin_service::*;
use user_profile_api::user_profile_management_service::*;
use user_profile_api::user_profile_registry_service::{
    UpdateSettingsRequest, UpdateSettingsResponse,
};

use protobuf::{EnumOrUnknown, MessageField};
use sdvmiddleware_rpc::RpcResult;
use std::collections::HashMap;
use std::sync::Mutex;
use std::vec::Vec;
use user_profile_api::user_profile_view::OnSettingsChangeRequest;

use user_profile_api::user_controllable_service;

use crate::macros::{acquire_lock_or_return_err, error_response};
use crate::setting::Setting;
use crate::settings_group::SettingsGroup;
use crate::vehicle_configuration::VehicleConfiguration;
use user_profile_utils::{print_setting_definition, print_settings};

use rand::Rng;

type Profiles = HashMap<u64, VehicleConfiguration>;
pub struct UserProfileServiceImpl {
    vehicle_state: Mutex<VehicleConfiguration>,
    user_profiles: Mutex<Profiles>,
    active_profile_id: Mutex<Option<u64>>,
    change_subscribers: Mutex<HashMap<String, HashSet<String>>>,
}

impl UserProfileServiceImpl {
    pub fn new() -> Self {
        UserProfileServiceImpl {
            vehicle_state: Mutex::new(VehicleConfiguration::new(User::new())),
            user_profiles: Mutex::new(Profiles::new()),
            active_profile_id: Mutex::new(None),
            change_subscribers: Mutex::new(HashMap::new()),
        }
    }

    fn apply_settings(group_name: String, settings: Vec<SettingProto>) -> Result<(), String> {
        // Ideally the client should be created via the get_user_controllable_service_client method
        // that is present in the UserProfileServer, for now manually create it since the rust
        // compiler cannot determine the correctness of the program if UserProfileServiceImpl & UserProfileServer
        // have references to each other
        let client: UserControllableServiceClient =
            UserControllableServiceClient::new(Box::new(GrpcClientTransport::new("HvacService")));

        let mut rng = rand::thread_rng();
        let transaction_id: u64 = rng.gen_range(0..1000);

        let resp = client
            .request_settings_change(user_controllable_service::RequestSettingsChangeRequest {
                group_name,
                transaction_id,
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

    fn notify_subscribers(settings: &str, setting: &[SettingProto], subscribers: &[String]) {
        // Ideally the client should be created via the get_user_controllable_service_client method
        // that is present in the UserProfileServer, for now manually create it since the rust
        // compiler cannot determine the correctness of the program if UserProfileServiceImpl & UserProfileServer
        // have references to each other
        for subscriber in subscribers {
            let client = UserProfileViewClient::new(Box::new(GrpcClientTransport::new(subscriber)));

            println!("Notifying {} of settings change\n", subscriber);
            client
                .on_settings_change(OnSettingsChangeRequest {
                    group: MessageField::some(SettingsGroupProto {
                        name: settings.to_string(),
                        settings: setting.to_vec(),
                        ..Default::default()
                    }),
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
                        setting.key, current_settings.name
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

impl UserProfileAdminService for UserProfileServiceImpl {
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

        let mut profiles = acquire_lock_or_return_err!(self.user_profiles, CreateUserResponse);
        if profiles.contains_key(&user.id) {
            println!("Invalid create user request, ID {} is already in use", user.id);
            return error_response!(
                CreateUserResponse,
                ResponseCode::INVALID_REQUEST,
                "User ID '{}' is already in use",
                user.id
            );
        }

        // Initialize profile with default values
        let vehicle_state = acquire_lock_or_return_err!(self.vehicle_state, CreateUserResponse);
        let mut new_profile = vehicle_state.clone();
        new_profile.reset_settings();
        new_profile.user = User { id: user.id, flags: user.flags, ..Default::default() };

        println!("Created profile with ID: {}", user.id);

        profiles.insert(user.id, new_profile);

        Ok(CreateUserResponse::new())
    }

    fn select_user(&self, request: SelectUserRequest) -> RpcResult<SelectUserResponse> {
        let mut profiles = acquire_lock_or_return_err!(self.user_profiles, SelectUserResponse);
        let mut active_profile_id =
            acquire_lock_or_return_err!(self.active_profile_id, SelectUserResponse);

        let target_profile = match profiles.get(&request.user_id) {
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

        println!("Loading profile of user {}\n", request.user_id);

        for settings_group in target_profile.settings_groups.values() {
            let settings = settings_group
                .settings
                .values()
                .filter(|x| x.kind.as_ref() == &SettingKind::PER_USER)
                .cloned()
                .map(SettingProto::from)
                .collect();

            if let Err(result) = Self::apply_settings(settings_group.name.clone(), settings) {
                println!("Failed to apply settings changes: {}", result);
            }
        }

        // If the previous profile is ephemeral, then delete it
        if let Some(id) =
            active_profile_id.filter(|&id| profiles.get(&id).map_or(false, |p| p.is_ephemeral()))
        {
            profiles.remove(&id);
            println!("Deleted ephemeral profile: {}", id);
        }
        *active_profile_id = Some(request.user_id);

        Ok(SelectUserResponse::new())
    }

    fn delete_user(&self, request: DeleteUserRequest) -> RpcResult<DeleteUserResponse> {
        let mut profiles = acquire_lock_or_return_err!(self.user_profiles, DeleteUserResponse);
        let active_profile_id =
            acquire_lock_or_return_err!(self.active_profile_id, DeleteUserResponse);

        if !profiles.contains_key(&request.user_id) {
            return Ok(DeleteUserResponse {
                error: MessageField::some(Error {
                    error_code: EnumOrUnknown::new(ResponseCode::NOT_FOUND_ERROR),
                    message: format!("User {} does not exist", request.user_id),
                    ..Default::default()
                }),
                ..Default::default()
            });
        }
        if active_profile_id.is_some_and(|id| id == request.user_id) {
            return Ok(DeleteUserResponse {
                error: MessageField::some(Error {
                    error_code: EnumOrUnknown::new(ResponseCode::INVALID_REQUEST),
                    message: format!("Cannot delete User {} since it is in use", request.user_id),
                    ..Default::default()
                }),
                ..Default::default()
            });
        }

        profiles.remove(&request.user_id);
        println!("Deleted profile with ID: {}", &request.user_id);

        Ok(DeleteUserResponse::new())
    }

    fn factory_reset(&self, _request: FactoryResetRequest) -> RpcResult<FactoryResetResponse> {
        let mut profiles = acquire_lock_or_return_err!(self.user_profiles, FactoryResetResponse);
        profiles.clear();

        let mut active_profile_id =
            acquire_lock_or_return_err!(self.active_profile_id, FactoryResetResponse);
        *active_profile_id = None;

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
        let profiles = acquire_lock_or_return_err!(self.user_profiles, ListUsersResponse);
        let users = profiles.iter().map(|(_, config)| config.user.clone()).collect_vec();

        Ok(ListUsersResponse { users, ..Default::default() })
    }

    fn get_user_settings(
        &self,
        request: GetUserSettingsRequest,
    ) -> RpcResult<GetUserSettingsResponse> {
        let profiles = acquire_lock_or_return_err!(self.user_profiles, GetUserSettingsResponse);

        let profile = match profiles.get(&request.user_id) {
            Some(profile) => profile,
            None => {
                return error_response!(
                    GetUserSettingsResponse,
                    ResponseCode::INVALID_REQUEST,
                    "Profile {} does not exist",
                    request.user_id
                )
            }
        };

        return Ok(GetUserSettingsResponse {
            groups: profile.settings_groups.values().cloned().map_into().collect(),
            ..Default::default()
        });
    }
}

impl UserProfileManagementService for UserProfileServiceImpl {
    fn request_settings_change(
        &self,
        request: RequestSettingsChangeRequest,
    ) -> RpcResult<RequestSettingsChangeResponse> {
        let vehicle_state =
            acquire_lock_or_return_err!(self.vehicle_state, RequestSettingsChangeResponse);

        let current_settings = match vehicle_state.settings_groups.get(&request.group_name) {
            Some(settings) => settings,
            None => {
                return error_response!(
                    RequestSettingsChangeResponse,
                    ResponseCode::INVALID_REQUEST,
                    "Settings group {} not found",
                    request.group_name
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

        println!("Settings Change request to {} with the following settings: ", request.group_name);
        print_settings(&request.settings);

        let resp = match Self::apply_settings(request.group_name, request.settings) {
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
        change_subscribers
            .entry(request.group_name.clone())
            .or_default()
            .insert(request.service_bundle_name);

        if let Some(settings_group) = vehicle_state.settings_groups.get(&request.group_name) {
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

            println!("Successfully registered subscriber to settings: {}\n", request.group_name);
            return Ok(result);
        }

        println!(
            "Received request to subscribe to non existent settings: {:?}",
            request.group_name
        );

        error_response!(
            SubscribeToSettingsChangeAndGetSettingsResponse,
            ResponseCode::INVALID_REQUEST,
            "Settings {:?} does not exist",
            request.group_name
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

        if let Some(settings_group_subscribers) = change_subscribers.get_mut(&request.group_name) {
            settings_group_subscribers.remove(&request.service_bundle_name);
        } else {
            return error_response!(
                UnsubscribeToSettingsChangeResponse,
                ResponseCode::INVALID_REQUEST,
                "Settings group {} does not exist",
                request.group_name
            );
        }

        Ok(UnsubscribeToSettingsChangeResponse::new())
    }
}

impl UserProfileRegistryService for UserProfileServiceImpl {
    fn register_settings(
        &self,
        request: RegisterSettingsRequest,
    ) -> RpcResult<RegisterSettingsResponse> {
        let mut vehicle_state =
            acquire_lock_or_return_err!(self.vehicle_state, RegisterSettingsResponse);

        if !vehicle_state.settings_groups.contains_key(&request.group_name)
            || vehicle_state.settings_groups.get(&request.group_name).unwrap().version
                != request.version
        {
            println!("Registered settings {}", request.group_name);

            print_setting_definition(&request.settings_definitions);

            // Also register the settings in the vehicle state
            let settings_group =
                vehicle_state.settings_groups.entry(request.group_name.clone()).or_insert(
                    SettingsGroup::new(request.group_name.clone(), request.version.clone(), vec![]),
                );

            request.settings_definitions.iter().cloned().for_each(|setting_definition| {
                let setting = Setting::from(setting_definition);

                if !settings_group.settings.contains_key(&setting.name) {
                    settings_group.settings.insert(setting.name.clone(), setting);
                }
            });
        } else {
            println!(
                "Settings {} with version {} is already registered",
                request.group_name, request.version
            );
        }

        Ok(RegisterSettingsResponse::new())
    }

    fn update_settings(&self, request: UpdateSettingsRequest) -> RpcResult<UpdateSettingsResponse> {
        let mut vehicle_state =
            acquire_lock_or_return_err!(self.vehicle_state, UpdateSettingsResponse);
        let change_subscribers =
            acquire_lock_or_return_err!(self.change_subscribers, UpdateSettingsResponse);

        let service_settings_group =
            match vehicle_state.settings_groups.get_mut(&request.group_name) {
                Some(x) => x,
                None => {
                    return error_response!(
                        UpdateSettingsResponse,
                        ResponseCode::NOT_FOUND_ERROR,
                        "Settings with {} not found",
                        request.group_name
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

        let active_profile_id =
            acquire_lock_or_return_err!(self.active_profile_id, UpdateSettingsResponse);

        if let Some(active_profile_id) = *active_profile_id {
            let mut profiles =
                acquire_lock_or_return_err!(self.user_profiles, UpdateSettingsResponse);
            let active_profile = profiles.get_mut(&active_profile_id).unwrap();
            let current_settings_group =
                active_profile.settings_groups.get_mut(&request.group_name).unwrap();

            request.settings.iter().cloned().for_each(|x| {
                if let Some(setting) = current_settings_group.settings.get_mut(&x.key) {
                    if let Err(err) = setting.set_value(x.value.unwrap()) {
                        println!("Cannot update setting {}: ", err);
                    }
                }
            });
        }

        Self::notify_subscribers(
            &request.group_name,
            &request.settings,
            &change_subscribers
                .get(&request.group_name)
                .unwrap_or(&HashSet::new())
                .iter()
                .cloned()
                .collect_vec(),
        );

        Ok(UpdateSettingsResponse::new())
    }
}
