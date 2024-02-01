// Copyright 2023 Google LLC
use sdvgenerated::user_profile_registry_service_interface::UserProfileRegistryService;

use user_profile_api::user_profile_registry_service::{
    RegisterConfigRequest, RegisterConfigResponse,
};

use user_profile_api::error::{Error, ResponseCode};
use user_profile_api::user_profile_registry_service::{UpdateConfigRequest, UpdateConfigResponse};

use protobuf::{EnumOrUnknown, MessageField};
use sdvmiddleware_rpc::RpcResult;
use std::collections::HashMap;
use std::sync::Mutex;

use crate::config_id::ConfigId;
use crate::config_metadata::ConfigMetadata;

pub struct UserProfileServiceImpl {
    defaults: Mutex<HashMap<ConfigId, ConfigMetadata>>,
}

impl UserProfileServiceImpl {
    pub fn new() -> Self {
        UserProfileServiceImpl { defaults: Mutex::new(HashMap::new()) }
    }
}

impl UserProfileServiceImpl {
    fn error(error_code: ResponseCode, message: String) -> MessageField<Error> {
        MessageField::some(Error {
            error_code: EnumOrUnknown::from(error_code),
            message,
            ..Default::default()
        })
    }
}

macro_rules! acquire_lock_or_return_err {
    ($mutex:expr,$return_type:ident) => {{
        match $mutex.lock() {
            Ok(x) => x,
            Err(e) => {
                return Ok($return_type {
                    error: UserProfileServiceImpl::error(
                        ResponseCode::RUNTIME_ERROR,
                        format!("Unable to obtain mutex: {:?}", e),
                    ),
                    ..Default::default()
                })
            }
        }
    }};
}

impl UserProfileRegistryService for UserProfileServiceImpl {
    fn register_config(&self, request: RegisterConfigRequest) -> RpcResult<RegisterConfigResponse> {
        println!("Received request to register configuration '{}'", request.config_name);

        let mut defaults = acquire_lock_or_return_err!(self.defaults, RegisterConfigResponse);

        let config_id = ConfigId::new(request.config_name.clone());
        let new_config_metadata = ConfigMetadata::new(request.version, request.settings);

        if !defaults.contains_key(&config_id)
            || defaults.get(&config_id).unwrap().version() != new_config_metadata.version()
        {
            println!("Registered config {}: {:#?}", config_id, new_config_metadata);
            defaults.insert(config_id, new_config_metadata);
        } else {
            println!(
                "Config {} with version {} is already registered",
                config_id,
                new_config_metadata.version()
            );
        }

        Ok(RegisterConfigResponse::new())
    }

    fn update_config(&self, _request: UpdateConfigRequest) -> RpcResult<UpdateConfigResponse> {
        todo!()
    }
}
