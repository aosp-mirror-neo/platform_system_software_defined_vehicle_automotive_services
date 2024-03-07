// Copyright 2023 Google LLC
use sdvgenerated::user_profile_management_service_interface::UserProfileManagementService;
use sdvgenerated::user_profile_registry_service_interface::UserProfileRegistryService;

use user_profile_api::configuration::Configuration;
use user_profile_api::user_profile_registry_service::{
    RegisterConfigRequest, RegisterConfigResponse,
};

use user_profile_api::error::{Error, ResponseCode};
use user_profile_api::setting::setting::Value;
use user_profile_api::setting::Setting;
use user_profile_api::user_profile_management_service::*;
use user_profile_api::user_profile_registry_service::{UpdateConfigRequest, UpdateConfigResponse};

use protobuf::{EnumOrUnknown, MessageField};
use sdvmiddleware_rpc::RpcResult;
use std::collections::HashMap;
use std::sync::Mutex;

use crate::config_metadata::ConfigMetadata;
use crate::macros::{acquire_lock_or_return_err, error_response};

type Settings = HashMap<String, Value>;
type Configurations = HashMap<String, Settings>;
type Profiles = HashMap<u64, Configurations>;

pub struct UserProfileServiceImpl {
    defaults: Mutex<HashMap<String, ConfigMetadata>>,
    vehicle_state: Mutex<HashMap<String, Settings>>,
    user_profiles: Mutex<Profiles>,
}

impl UserProfileServiceImpl {
    pub fn new() -> Self {
        UserProfileServiceImpl {
            defaults: Mutex::new(HashMap::new()),
            vehicle_state: Mutex::new(HashMap::new()),
            user_profiles: Mutex::new(Profiles::new()),
        }
    }
}

impl UserProfileManagementService for UserProfileServiceImpl {
    fn create_user(&self, request: CreateUserRequest) -> RpcResult<CreateUserResponse> {
        if request.id == 0 {
            println!(
                "Invalid create user request, ID {} is reserved for guest profile",
                request.id
            );
            return error_response!(
                CreateUserResponse,
                ResponseCode::INVALID_REQUEST,
                "User ID '0' is reserved for guest profile",
            );
        }

        let mut profiles = acquire_lock_or_return_err!(self.user_profiles, CreateUserResponse);
        if profiles.contains_key(&request.id) {
            println!("Invalid create user request, ID {} is already in use", request.id);
            return error_response!(
                CreateUserResponse,
                ResponseCode::INVALID_REQUEST,
                "User ID '{}' is already in use",
                request.id
            );
        }

        let mut configurations = Configurations::new();
        for config in request.configs.iter() {
            let settings_map: HashMap<String, Value> = config
                .settings
                .iter()
                .map(|setting| (setting.key.clone(), setting.value.clone().unwrap()))
                .collect::<HashMap<_, _>>();

            configurations.insert(config.config_name.clone(), settings_map);
        }

        println!("Created profile with ID: {}", request.id);

        profiles.insert(request.id, configurations);

        Ok(CreateUserResponse::new())
    }

    fn select_user(&self, _request: SelectUserRequest) -> RpcResult<SelectUserResponse> {
        todo!()
    }

    fn delete_user(&self, _request: DeleteUserRequest) -> RpcResult<DeleteUserResponse> {
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

    fn factory_reset(&self, _request: FactoryResetRequest) -> RpcResult<FactoryResetResponse> {
        todo!()
    }

    fn request_config_change(
        &self,
        _request: RequestConfigChangeRequest,
    ) -> RpcResult<RequestConfigChangeResponse> {
        todo!()
    }

    fn subscribe_to_config_change(
        &self,
        request: SubscribeToConfigChangeRequest,
    ) -> RpcResult<SubscribeToConfigChangeResponse> {
        //TODO: Once middleware exposes the identity of the caller, we need to
        //register their identity. For now we will rely on a client that is hardcoded to call a specific service bundle
        let vehicle_state =
            acquire_lock_or_return_err!(self.vehicle_state, SubscribeToConfigChangeResponse);

        if let Some(settings) = vehicle_state.get(&request.config_name) {
            let mut result = SubscribeToConfigChangeResponse::new();
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

            println!("Successfully registered subscriber to config: {:?}", request.config_name);
            return Ok(result);
        }

        println!("Received request to subscribe to non existent config: {:?}", request.config_name);

        error_response!(
            SubscribeToConfigChangeResponse,
            ResponseCode::INVALID_REQUEST,
            "Config {:?} does not exist",
            request.config_name
        )
    }
}

impl UserProfileRegistryService for UserProfileServiceImpl {
    fn register_config(&self, request: RegisterConfigRequest) -> RpcResult<RegisterConfigResponse> {
        let mut defaults = acquire_lock_or_return_err!(self.defaults, RegisterConfigResponse);
        let mut vehicle_state =
            acquire_lock_or_return_err!(self.vehicle_state, RegisterConfigResponse);

        let new_config_metadata = ConfigMetadata::new(request.version, request.settings.clone());

        if !defaults.contains_key(&request.config_name)
            || defaults.get(&request.config_name).unwrap().version()
                != new_config_metadata.version()
        {
            println!("Registered config {}: {:#?}", request.config_name, new_config_metadata);
            defaults.insert(request.config_name.clone(), new_config_metadata);

            // Also register the settings in the vehicle state
            let service_config =
                vehicle_state.entry(request.config_name.clone()).or_insert(HashMap::new());
            request.settings.iter().cloned().for_each(|setting_definition| {
                let setting = setting_definition.setting.unwrap();
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

        println!("Updating config {}", request.config_name);
        let service_config: &mut Settings = match vehicle_state.get_mut(&request.config_name) {
            Some(x) => x,
            None => {
                return error_response!(
                    UpdateConfigResponse,
                    ResponseCode::NOT_FOUND_ERROR,
                    "Config with ID {:?} not found",
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
            println!("Set '{}' to '{:?}'", x.key, new_value);
            service_config.insert(x.key, new_value);
        });

        Ok(UpdateConfigResponse::new())
    }
}
