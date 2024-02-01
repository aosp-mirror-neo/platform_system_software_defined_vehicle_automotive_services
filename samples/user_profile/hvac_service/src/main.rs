// Copyright 2024 Google LLC
#![allow(missing_docs, dead_code, unused_variables, unused_imports)]

mod hvac_settings;

use protobuf::EnumOrUnknown;
use protobuf::MessageField;
use sdvgenerated::hvac_service::HvacService;
use sdvgenerated::user_profile_registry_service_interface::UserProfileRegistryService;
use user_profile_api::setting::Setting;
use user_profile_api::user_profile_registry_service::RegisterConfigRequest;
use user_profile_api::user_profile_registry_service::RegisterConfigResponse;
use user_profile_api::user_profile_registry_service::SettingDefinition;
use user_profile_api::user_profile_registry_service::SettingKind;

fn main() {
    let hvac_service = HvacService::new();

    let mut register_config_request = RegisterConfigRequest::new();
    register_config_request.config_name = String::from("hvac");
    register_config_request.version = String::from("1.0");
    register_config_request.settings = hvac_settings::default_configuration()
        .iter()
        .map(|(key, value)| SettingDefinition {
            type_: EnumOrUnknown::new(SettingKind::PER_USER),
            setting: MessageField::some(Setting {
                key: key.clone(),
                value: Some(value.clone()),
                ..Default::default()
            }),
            ..Default::default()
        })
        .collect();

    println!(
        "Registering config with ID: {},\nVersion: {}\nSettings: {:#?}",
        register_config_request.config_name,
        register_config_request.version,
        register_config_request.settings
    );

    let response = hvac_service
        .user_profile_registry_service_client
        .register_config(register_config_request.clone())
        .expect("Failed to perform GRPC request to register default configuration");
    if response.error.is_some() {
        panic!("Failed to register configuration: {:#?}", response.error.unwrap());
    }

    println!("Registering the same configuration again");

    let response = hvac_service
        .user_profile_registry_service_client
        .register_config(register_config_request)
        .expect("Failed to perform GRPC request to re-register configuration");
    if response.error.is_some() {
        panic!("Failed to register configuration: {:#?}", response.error.unwrap());
    }
}
