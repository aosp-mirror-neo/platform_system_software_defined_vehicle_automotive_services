// Copyright 2023 Google LLC
use itertools::Itertools;
use sdvgenerated::user_profile_admin_service_interface::UserProfileAdminService;
use sdvgenerated::user_profile_management_service_interface::UserProfileManagementService;
use sdvgenerated::user_profile_registry_service_interface::UserProfileRegistryService;
use sdvgenerated_rpc_user_controllable_service::client::UserControllableServiceClient;
use sdvgenerated_rpc_user_controllable_service::interface::UserControllableService;

use sdvmiddleware_rpc_grpc::grpc_client_transport::GrpcClientTransport;

use user_profile_api::configuration::Configuration;
use user_profile_api::user_profile_registry_service::{
    RegisterConfigRequest, RegisterConfigResponse,
};

use user_profile_api::error::{Error, ResponseCode};
use user_profile_api::setting::setting::Value;
use user_profile_api::setting::{self, Setting, SettingKind};
use user_profile_api::user_profile_admin_service::*;
use user_profile_api::user_profile_management_service::*;
use user_profile_api::user_profile_registry_service::{UpdateConfigRequest, UpdateConfigResponse};

use protobuf::{EnumOrUnknown, MessageField};
use sdvmiddleware_rpc::RpcResult;
use std::collections::HashMap;
use std::sync::Mutex;
use std::vec::Vec;

use user_profile_api::user_controllable_service;

use crate::config_metadata::ConfigMetadata;
use crate::macros::{acquire_lock_or_return_err, error_response};
use user_profile_utils::{print_setting_definition, print_settings};

use rand::Rng;

type Settings = HashMap<String, Value>;
type Configurations = HashMap<String, Settings>;
type Profiles = HashMap<u64, Configurations>;

pub struct UserProfileServiceImpl {
    defaults: Mutex<HashMap<String, ConfigMetadata>>,
    vehicle_state: Mutex<Configurations>,
    user_profiles: Mutex<Profiles>,
    active_profile: Mutex<Option<u64>>,
}

impl UserProfileServiceImpl {
    pub fn new() -> Self {
        UserProfileServiceImpl {
            defaults: Mutex::new(HashMap::new()),
            vehicle_state: Mutex::new(HashMap::new()),
            user_profiles: Mutex::new(Profiles::new()),
            active_profile: Mutex::new(None),
        }
    }

    fn apply_config(config_name: String, settings: Vec<setting::Setting>) -> Result<(), String> {
        let client: UserControllableServiceClient = UserControllableServiceClient::new(Box::new(
            GrpcClientTransport::new("HvacService::user_controllable_service"),
        ));

        let mut rng = rand::thread_rng();
        let transaction_id: u64 = rng.gen_range(0..1000);

        let resp = client
            .request_config_change(user_controllable_service::RequestConfigChangeRequest {
                config_name,
                transaction_id,
                settings,
                ..Default::default()
            })
            .map_err(|x| {
                format!("Failed to perform GRPC request to request_config_change: {}", x)
            })?;

        match resp.error.is_some() {
            true => Err(format!("Failed to request config changes: {}", resp.error.unwrap())),
            false => Ok(()),
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
        let mut configurations = Configurations::new();
        let defaults = acquire_lock_or_return_err!(self.defaults, CreateUserResponse);

        for (config_name, config_defaults) in defaults.iter() {
            let settings_map: HashMap<String, Value> = config_defaults
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

            configurations.insert(config_name.clone(), settings_map);
        }

        // override defaults with values provided by user creator
        for config in request.configs.iter() {
            let service_config = configurations.entry(config.config_name.clone()).or_default();

            config.settings.iter().for_each(|setting| {
                service_config.insert(setting.key.clone(), setting.value.clone().unwrap());
            });
        }

        println!("Created profile with ID: {}", user.id);

        profiles.insert(user.id, configurations);

        Ok(CreateUserResponse::new())
    }

    fn select_user(&self, request: SelectUserRequest) -> RpcResult<SelectUserResponse> {
        let profiles = acquire_lock_or_return_err!(self.user_profiles, SelectUserResponse);
        let mut active_profile =
            acquire_lock_or_return_err!(self.active_profile, SelectUserResponse);

        let target_profile: &Configurations = match profiles.get(&request.user_id) {
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

        for (config_name, settings) in target_profile {
            let settings = settings
                .iter()
                .map(|(setting_name, value)| setting::Setting {
                    key: setting_name.clone(),
                    value: Some(value.clone()),
                    ..Default::default()
                })
                .collect_vec();

            if let Err(result) = Self::apply_config(config_name.clone(), settings) {
                println!("Failed to apply config changes: {}", result);
            }
        }

        Ok(SelectUserResponse::new())
    }

    fn delete_user(&self, _request: DeleteUserRequest) -> RpcResult<DeleteUserResponse> {
        todo!()
    }

    fn factory_reset(&self, _request: FactoryResetRequest) -> RpcResult<FactoryResetResponse> {
        todo!()
    }

    fn list_users(&self, _request: ListUsersRequest) -> RpcResult<ListUsersResponse> {
        todo!()
    }

    fn get_user_configs(
        &self,
        _request: GetUserConfigsRequest,
    ) -> RpcResult<GetUserConfigsResponse> {
        todo!()
    }
}

impl UserProfileManagementService for UserProfileServiceImpl {
    fn request_config_change(
        &self,
        request: RequestConfigChangeRequest,
    ) -> RpcResult<RequestConfigChangeResponse> {
        let vehicle_state =
            acquire_lock_or_return_err!(self.vehicle_state, RequestConfigChangeResponse);

        if !vehicle_state.contains_key(&request.config_name) {
            return error_response!(
                RequestConfigChangeResponse,
                ResponseCode::INVALID_REQUEST,
                "Config {} not found",
                request.config_name
            );
        }

        println!("Config Change request to {} with the following settings: ", request.config_name);
        print_settings(&request.settings);

        let resp = match Self::apply_config(request.config_name, request.settings) {
            Ok(_) => RequestConfigChangeResponse::new(),
            Err(err) => RequestConfigChangeResponse {
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

    fn subscribe_to_config_change_and_get_config(
        &self,
        request: SubscribeToConfigChangeAndGetConfigRequest,
    ) -> RpcResult<SubscribeToConfigChangeAndGetConfigResponse> {
        //TODO: Once middleware exposes the identity of the caller, we need to
        //register their identity. For now we will rely on a client that is hardcoded to call a specific service bundle
        let vehicle_state = acquire_lock_or_return_err!(
            self.vehicle_state,
            SubscribeToConfigChangeAndGetConfigResponse
        );

        if let Some(settings) = vehicle_state.get(&request.config_name) {
            let mut result = SubscribeToConfigChangeAndGetConfigResponse::new();
            result.config = MessageField::some(Configuration {
                config_name: request.config_name.clone(),
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

            println!("Successfully registered subscriber to config: {}\n", request.config_name);
            return Ok(result);
        }

        println!("Received request to subscribe to non existent config: {:?}", request.config_name);

        error_response!(
            SubscribeToConfigChangeAndGetConfigResponse,
            ResponseCode::INVALID_REQUEST,
            "Config {:?} does not exist",
            request.config_name
        )
    }

    fn unsubscribe_from_config_change(
        &self,
        _request: UnsubscribeToConfigChangeRequest,
    ) -> RpcResult<UnsubscribeToConfigChangeResponse> {
        todo!()
    }
}

impl UserProfileRegistryService for UserProfileServiceImpl {
    fn register_config(&self, request: RegisterConfigRequest) -> RpcResult<RegisterConfigResponse> {
        let mut defaults = acquire_lock_or_return_err!(self.defaults, RegisterConfigResponse);
        let mut vehicle_state =
            acquire_lock_or_return_err!(self.vehicle_state, RegisterConfigResponse);

        let new_config_metadata =
            ConfigMetadata::new(request.version, request.settings_definitions.clone());

        if !defaults.contains_key(&request.config_name)
            || defaults.get(&request.config_name).unwrap().version()
                != new_config_metadata.version()
        {
            println!("Registered config {}", request.config_name);

            print_setting_definition(&request.settings_definitions);

            defaults.insert(request.config_name.clone(), new_config_metadata);

            // Also register the settings in the vehicle state
            let service_config =
                vehicle_state.entry(request.config_name.clone()).or_insert(HashMap::new());

            request.settings_definitions.iter().cloned().for_each(|setting_definition| {
                let setting_and_constraints = setting_definition.setting_and_constaints.unwrap();
                let setting = setting_and_constraints.setting.unwrap();

                service_config.entry(setting.key).or_insert(setting.value.unwrap());
            });
        } else {
            println!(
                "Config {} with version {} is already registered",
                request.config_name,
                new_config_metadata.version()
            );
        }

        Ok(RegisterConfigResponse::new())
    }

    fn update_config(&self, request: UpdateConfigRequest) -> RpcResult<UpdateConfigResponse> {
        let mut vehicle_state =
            acquire_lock_or_return_err!(self.vehicle_state, UpdateConfigResponse);

        let service_config: &mut Settings = match vehicle_state.get_mut(&request.config_name) {
            Some(x) => x,
            None => {
                return error_response!(
                    UpdateConfigResponse,
                    ResponseCode::NOT_FOUND_ERROR,
                    "Config with {} not found",
                    request.config_name
                )
            }
        };

        if request.settings.iter().any(|setting| setting.value.is_none()) {
            return error_response!(
                UpdateConfigResponse,
                ResponseCode::INVALID_REQUEST,
                "Encountered settings with a None value: {:?}",
                request.settings
            );
        }

        request.settings.iter().cloned().for_each(|x| {
            let new_value = x.value.unwrap();
            service_config.insert(x.key, new_value);
        });

        println!("Updated the following settings");
        print_settings(&request.settings);

        let active_profile_id =
            acquire_lock_or_return_err!(self.active_profile, UpdateConfigResponse);

        if let Some(active_profile_id) = *active_profile_id {
            let mut profiles =
                acquire_lock_or_return_err!(self.user_profiles, UpdateConfigResponse);
            let active_profile: &mut HashMap<String, HashMap<String, Value>> =
                profiles.get_mut(&active_profile_id).unwrap();
            let current_config: &mut HashMap<String, Value> =
                active_profile.get_mut(&request.config_name).unwrap();

            request.settings.iter().cloned().for_each(|x| {
                let new_value = x.value.unwrap();
                current_config.insert(x.key, new_value);
            });
        }

        Ok(UpdateConfigResponse::new())
    }
}
