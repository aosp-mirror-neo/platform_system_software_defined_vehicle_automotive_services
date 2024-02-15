// Copyright 2024 Google LLC
#![allow(missing_docs)]

mod settings;
mod user_controllable_service_impl;

use std::sync::Arc;

use protobuf::EnumOrUnknown;
use protobuf::MessageField;
use sdvgenerated::hvac_service::HvacService;
use sdvgenerated::user_profile_registry_service_interface::UserProfileRegistryService;
use user_profile_api::setting::SettingAndConstraints;
use user_profile_api::setting::{Setting, SettingDefinition, SettingKind};
use user_profile_api::user_profile_registry_service::RegisterConfigRequest;

use crate::settings::{default_configuration, CONFIG_NAME};
use crate::user_controllable_service_impl::UserControllableServiceImpl;
use user_profile_utils::print_setting_definition;

fn main() {
    let user_controllable_service_impl = UserControllableServiceImpl::new();

    let hvac_service = HvacService::new();
    hvac_service
        .user_controllable_service_server
        .register_service_interface(Arc::new(user_controllable_service_impl));

    let mut register_config_request = RegisterConfigRequest::new();
    register_config_request.config_name = CONFIG_NAME.to_string();
    register_config_request.version = String::from("1.0");

    register_config_request.settings_definitions = default_configuration()
        .iter()
        .map(|(key, value)| SettingDefinition {
            type_: EnumOrUnknown::new(SettingKind::PER_USER),
            setting_and_constaints: MessageField::some(SettingAndConstraints {
                setting: MessageField::some(Setting {
                    key: key.clone(),
                    value: Some(value.clone()),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        })
        .collect();

    println!(
        "Registering config with name: {},\nVersion: {}",
        register_config_request.config_name, register_config_request.version,
    );

    print_setting_definition(&register_config_request.settings_definitions);

    let registry_service_client =
        hvac_service.get_user_profile_registry_service_client("UserProfileServer");

    let response = registry_service_client
        .register_config(register_config_request.clone())
        .expect("Failed to perform GRPC request to register default configuration");
    if response.error.is_some() {
        panic!("Failed to register configuration: {:#?}", response.error.unwrap());
    }

    println!("Registering the same configuration again\n\n");

    let response = registry_service_client
        .register_config(register_config_request)
        .expect("Failed to perform GRPC request to re-register configuration");
    if response.error.is_some() {
        panic!("Failed to register configuration: {:#?}", response.error.unwrap());
    }

    loop {
        std::thread::park();
    }
}
