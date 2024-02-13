// Copyright 2023 Google LLC
use sdvgenerated::user_profile_registry_service_interface::UserProfileRegistryService;

use user_profile_api::user_profile_registry_service::{
    RegisterConfigRequest, RegisterConfigResponse,
};

use user_profile_api::error::{Error, ResponseCode};
use user_profile_api::setting::setting::Value;
use user_profile_api::user_profile_registry_service::{UpdateConfigRequest, UpdateConfigResponse};

use protobuf::{EnumOrUnknown, MessageField};
use sdvmiddleware_rpc::RpcResult;
use std::collections::HashMap;
use std::sync::Mutex;

use crate::config_id::ConfigId;
use crate::config_metadata::ConfigMetadata;
use crate::macros::{acquire_lock_or_return_err, error_response};

type Settings = HashMap<String, Value>;

pub struct UserProfileServiceImpl {
    defaults: Mutex<HashMap<ConfigId, ConfigMetadata>>,
    vehicle_state: Mutex<HashMap<ConfigId, Settings>>,
}

impl UserProfileServiceImpl {
    pub fn new() -> Self {
        UserProfileServiceImpl {
            defaults: Mutex::new(HashMap::new()),
            vehicle_state: Mutex::new(HashMap::new()),
        }
    }
}

impl UserProfileRegistryService for UserProfileServiceImpl {
    fn register_config(&self, request: RegisterConfigRequest) -> RpcResult<RegisterConfigResponse> {
        let mut defaults = acquire_lock_or_return_err!(self.defaults, RegisterConfigResponse);
        let mut vehicle_state =
            acquire_lock_or_return_err!(self.vehicle_state, RegisterConfigResponse);

        let config_id = ConfigId::new(request.config_name.clone());
        let new_config_metadata = ConfigMetadata::new(request.version, request.settings.clone());

        if !defaults.contains_key(&config_id)
            || defaults.get(&config_id).unwrap().version() != new_config_metadata.version()
        {
            println!("Registered config {}: {:#?}", config_id, new_config_metadata);
            defaults.insert(config_id.clone(), new_config_metadata);

            // Also register the settings in the vehicle state
            let service_config = vehicle_state.entry(config_id.clone()).or_insert(HashMap::new());
            request.settings.iter().cloned().for_each(|setting_definition| {
                let setting = setting_definition.setting.unwrap();
                service_config.entry(setting.key).or_insert(setting.value.unwrap());
            });
        } else {
            println!(
                "Config {} with version {} is already registered",
                config_id,
                new_config_metadata.version()
            );
        }

        Ok(RegisterConfigResponse::new())
    }

    fn update_config(&self, request: UpdateConfigRequest) -> RpcResult<UpdateConfigResponse> {
        let mut vehicle_state =
            acquire_lock_or_return_err!(self.vehicle_state, UpdateConfigResponse);

        println!("Updating config {}", request.config_name);
        let config_id = ConfigId::new(request.config_name.clone());
        let service_config: &mut Settings = match vehicle_state.get_mut(&config_id) {
            Some(x) => x,
            None => {
                return error_response!(
                    UpdateConfigResponse,
                    ResponseCode::NOT_FOUND_ERROR,
                    "Config with ID {:?} not found",
                    config_id
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
