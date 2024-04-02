// Copyright 2023 Google LLC
use itertools::Itertools;
use sdvgenerated::user_profile_admin_service_interface::UserProfileAdminService;
use sdvgenerated::user_profile_management_service_interface::UserProfileManagementService;
use sdvgenerated::user_profile_registry_service_interface::UserProfileRegistryService;
use sdvgenerated_rpc_user_controllable_service::client::UserControllableServiceClient;
use sdvgenerated_rpc_user_controllable_service::interface::UserControllableService;
use sdvgenerated_rpc_user_profile_view::interface::UserProfileView;

use sdvgenerated_rpc_user_profile_view::client::UserProfileViewClient;
use sdvmiddleware_rpc_grpc::grpc_client_transport::GrpcClientTransport;

use user_profile_api::settings_group::SettingsGroup;
use user_profile_api::user_profile_registry_service::{
    RegisterSettingsRequest, RegisterSettingsResponse,
};

use user_profile_api::error::{Error, ResponseCode};
use user_profile_api::setting::setting::Value;
use user_profile_api::setting::{self, Setting, SettingKind};
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
use crate::settings_metadata::SettingsGroupMetadata;
use user_profile_utils::{print_setting_definition, print_settings};

use rand::Rng;

type Settings = HashMap<String, Value>;
type SettingsGroupTable = HashMap<String, Settings>;
type Profiles = HashMap<u64, SettingsGroupTable>;

pub struct UserProfileServiceImpl {
    defaults: Mutex<HashMap<String, SettingsGroupMetadata>>,
    vehicle_state: Mutex<SettingsGroupTable>,
    user_profiles: Mutex<Profiles>,
    active_profile: Mutex<Option<u64>>,
    change_subscribers: Mutex<HashMap<String, Vec<String>>>,
}

impl UserProfileServiceImpl {
    pub fn new() -> Self {
        UserProfileServiceImpl {
            defaults: Mutex::new(HashMap::new()),
            vehicle_state: Mutex::new(HashMap::new()),
            user_profiles: Mutex::new(Profiles::new()),
            active_profile: Mutex::new(None),
            change_subscribers: Mutex::new(HashMap::new()),
        }
    }

    fn apply_settings(group_name: String, settings: Vec<setting::Setting>) -> Result<(), String> {
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

    fn notify_subscribers(settings: &str, setting: &[Setting], subscribers: &[String]) {
        // Ideally the client should be created via the get_user_controllable_service_client method
        // that is present in the UserProfileServer, for now manually create it since the rust
        // compiler cannot determine the correctness of the program if UserProfileServiceImpl & UserProfileServer
        // have references to each other
        for subscriber in subscribers {
            let client = UserProfileViewClient::new(Box::new(GrpcClientTransport::new(subscriber)));

            println!("Notifying {} of settings change\n", subscriber);
            client
                .on_settings_change(OnSettingsChangeRequest {
                    group: MessageField::some(SettingsGroup {
                        name: settings.to_string(),
                        settings: setting.to_vec(),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .expect("Failed to notify clients of settings changes");
        }
    }
}

impl UserProfileAdminService for UserProfileServiceImpl {
    fn create_user(&self, request: CreateUserRequest) -> RpcResult<CreateUserResponse> {
        //TODO(b/328376446): Add support for guest user
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
        let mut setting_groups = SettingsGroupTable::new();
        let defaults = acquire_lock_or_return_err!(self.defaults, CreateUserResponse);

        for (group_name, default_settings) in defaults.iter() {
            let settings_map: HashMap<String, Value> = default_settings
                .settings_definition()
                .iter()
                .filter(|x| x.type_.unwrap() == SettingKind::PER_USER)
                .cloned()
                .map(|setting_definition| {
                    let setting_and_constraints =
                        setting_definition.setting_and_constaints.unwrap();
                    let setting = setting_and_constraints.setting.unwrap();

                    (setting.key.clone(), setting.value.clone().unwrap())
                })
                .collect::<HashMap<_, _>>();

            setting_groups.insert(group_name.clone(), settings_map);
        }

        println!("Created profile with ID: {}", user.id);

        profiles.insert(user.id, setting_groups);

        Ok(CreateUserResponse::new())
    }

    fn select_user(&self, request: SelectUserRequest) -> RpcResult<SelectUserResponse> {
        let profiles = acquire_lock_or_return_err!(self.user_profiles, SelectUserResponse);
        let mut active_profile =
            acquire_lock_or_return_err!(self.active_profile, SelectUserResponse);

        let target_profile: &SettingsGroupTable = match profiles.get(&request.user_id) {
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
        *active_profile = Some(request.user_id);

        for (group_name, settings) in target_profile {
            let settings = settings
                .iter()
                .map(|(setting_name, value)| setting::Setting {
                    key: setting_name.clone(),
                    value: Some(value.clone()),
                    ..Default::default()
                })
                .collect_vec();

            if let Err(result) = Self::apply_settings(group_name.clone(), settings) {
                println!("Failed to apply settings changes: {}", result);
            }
        }

        Ok(SelectUserResponse::new())
    }

    fn delete_user(&self, request: DeleteUserRequest) -> RpcResult<DeleteUserResponse> {
        let mut profiles = acquire_lock_or_return_err!(self.user_profiles, DeleteUserResponse);
        let active_profile = acquire_lock_or_return_err!(self.active_profile, DeleteUserResponse);

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
        if active_profile.is_some_and(|id| id == request.user_id) {
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

        Ok(DeleteUserResponse::new())
    }

    fn factory_reset(&self, _request: FactoryResetRequest) -> RpcResult<FactoryResetResponse> {
        let mut profiles = acquire_lock_or_return_err!(self.user_profiles, FactoryResetResponse);
        profiles.clear();

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
        todo!()
    }

    fn get_user_settings(
        &self,
        _request: GetUserSettingsRequest,
    ) -> RpcResult<GetUserSettingsResponse> {
        todo!()
    }
}

impl UserProfileManagementService for UserProfileServiceImpl {
    fn request_settings_change(
        &self,
        request: RequestSettingsChangeRequest,
    ) -> RpcResult<RequestSettingsChangeResponse> {
        let vehicle_state =
            acquire_lock_or_return_err!(self.vehicle_state, RequestSettingsChangeResponse);

        if !vehicle_state.contains_key(&request.group_name) {
            return error_response!(
                RequestSettingsChangeResponse,
                ResponseCode::INVALID_REQUEST,
                "Settings {} not found",
                request.group_name
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
        //TODO: Once middleware exposes the identity of the caller, we need to
        //register their identity. For now we will rely on a client that is hardcoded to call a specific service bundle
        change_subscribers.insert(request.group_name.clone(), vec![String::from("HmiService")]);

        if let Some(settings) = vehicle_state.get(&request.group_name) {
            let mut result = SubscribeToSettingsChangeAndGetSettingsResponse::new();
            result.group = MessageField::some(SettingsGroup {
                name: request.group_name.clone(),
                settings: settings
                    .iter()
                    .map(|(key, value)| Setting {
                        key: key.clone(),
                        value: Some(value.clone()),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            });

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
        _request: UnsubscribeToSettingsChangeRequest,
    ) -> RpcResult<UnsubscribeToSettingsChangeResponse> {
        todo!()
    }
}

impl UserProfileRegistryService for UserProfileServiceImpl {
    fn register_settings(
        &self,
        request: RegisterSettingsRequest,
    ) -> RpcResult<RegisterSettingsResponse> {
        let mut defaults = acquire_lock_or_return_err!(self.defaults, RegisterSettingsResponse);
        let mut vehicle_state =
            acquire_lock_or_return_err!(self.vehicle_state, RegisterSettingsResponse);

        let settings_group_metadata =
            SettingsGroupMetadata::new(request.version, request.settings_definitions.clone());

        if !defaults.contains_key(&request.group_name)
            || defaults.get(&request.group_name).unwrap().version()
                != settings_group_metadata.version()
        {
            println!("Registered settings {}", request.group_name);

            print_setting_definition(&request.settings_definitions);

            defaults.insert(request.group_name.clone(), settings_group_metadata);

            // Also register the settings in the vehicle state
            let service_settings =
                vehicle_state.entry(request.group_name.clone()).or_insert(HashMap::new());

            request.settings_definitions.iter().cloned().for_each(|setting_definition| {
                let setting_and_constraints = setting_definition.setting_and_constaints.unwrap();
                let setting = setting_and_constraints.setting.unwrap();

                service_settings.entry(setting.key).or_insert(setting.value.unwrap());
            });
        } else {
            println!(
                "Settings {} with version {} is already registered",
                request.group_name,
                settings_group_metadata.version()
            );
        }

        Ok(RegisterSettingsResponse::new())
    }

    fn update_settings(&self, request: UpdateSettingsRequest) -> RpcResult<UpdateSettingsResponse> {
        let mut vehicle_state =
            acquire_lock_or_return_err!(self.vehicle_state, UpdateSettingsResponse);
        let change_subscribers =
            acquire_lock_or_return_err!(self.change_subscribers, UpdateSettingsResponse);

        let service_settings: &mut Settings = match vehicle_state.get_mut(&request.group_name) {
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
            let new_value = x.value.unwrap();
            service_settings.insert(x.key, new_value);
        });

        println!("Updated the following settings");
        print_settings(&request.settings);

        let active_profile_id =
            acquire_lock_or_return_err!(self.active_profile, UpdateSettingsResponse);

        if let Some(active_profile_id) = *active_profile_id {
            let mut profiles =
                acquire_lock_or_return_err!(self.user_profiles, UpdateSettingsResponse);
            let active_profile: &mut HashMap<String, HashMap<String, Value>> =
                profiles.get_mut(&active_profile_id).unwrap();
            let current_settings: &mut HashMap<String, Value> =
                active_profile.get_mut(&request.group_name).unwrap();

            request.settings.iter().cloned().for_each(|x| {
                let new_value = x.value.unwrap();
                current_settings.insert(x.key, new_value);
            });
        }

        Self::notify_subscribers(
            &request.group_name,
            &request.settings,
            change_subscribers.get(&request.group_name).unwrap_or(&vec![]),
        );

        Ok(UpdateSettingsResponse::new())
    }
}
