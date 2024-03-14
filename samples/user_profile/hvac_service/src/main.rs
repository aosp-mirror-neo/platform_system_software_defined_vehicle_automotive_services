// Copyright 2024 Google LLC
#![allow(missing_docs)]

mod hvac_settings;

use protobuf::EnumOrUnknown;
use protobuf::MessageField;
use sdvgenerated::hvac_service::HvacService;
use sdvgenerated::user_profile_registry_service_interface::UserProfileRegistryService;
use user_profile_api::setting::setting::Value;
use user_profile_api::setting::SettingAndConstraints;
use user_profile_api::setting::{Setting, SettingDefinition, SettingKind};
use user_profile_api::user_profile_registry_service::RegisterConfigRequest;
use user_profile_api::user_profile_registry_service::UpdateConfigRequest;

fn main() {
    let hvac_service = HvacService::new();

    let mut register_config_request = RegisterConfigRequest::new();
    register_config_request.config_name = hvac_settings::CONFIG_NAME.to_string();
    register_config_request.version = String::from("1.0");

    register_config_request.settings_definitions = hvac_settings::default_configuration()
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
        "Registering config with ID: {},\nVersion: {}\nSettings: {:#?}",
        register_config_request.config_name,
        register_config_request.version,
        register_config_request.settings_definitions
    );

    let registry_service_client =
        hvac_service.get_user_profile_registry_service_client("UserProfileServer");

    let response = registry_service_client
        .register_config(register_config_request.clone())
        .expect("Failed to perform GRPC request to register default configuration");
    if response.error.is_some() {
        panic!("Failed to register configuration: {:#?}", response.error.unwrap());
    }

    println!("Registering the same configuration again");

    let response = registry_service_client
        .register_config(register_config_request)
        .expect("Failed to perform GRPC request to re-register configuration");
    if response.error.is_some() {
        panic!("Failed to register configuration: {:#?}", response.error.unwrap());
    }

    println!("Updating setting '{}' to 'true'", hvac_settings::ENABLED);
    let response: user_profile_api::user_profile_registry_service::UpdateConfigResponse =
        registry_service_client
            .update_config(UpdateConfigRequest {
                config_name: hvac_settings::CONFIG_NAME.to_string(),
                settings: vec![Setting {
                    key: hvac_settings::ENABLED.to_string(),
                    value: Some(Value::Bool(true)),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .expect("Failed to GRPC request to update config");
    if response.error.is_some() {
        panic!("Failed to update configuration: {:#?}", response.error.unwrap());
    }
}
