// Copyright 2024 Google LLC
#![allow(missing_docs)]

mod settings;
mod user_controllable_service_impl;

use std::sync::Arc;

use sdvgenerated::hvac_service::HvacService;
use sdvgenerated::user_preferences_registry_service_interface::UserPreferencesRegistryService;
use user_preferences_api::user_preferences_registry_service::RegisterSettingsRequest;

use crate::settings::{setting_definitions, SETTINGS_GROUP_NAME};
use crate::user_controllable_service_impl::UserControllableServiceImpl;
use user_preferences_utils::print_setting_definition;

pub(crate) const SERVICE_FQIN: &str = "HvacService";

fn main() {
    let user_controllable_service_impl = UserControllableServiceImpl::new();

    let hvac_service = HvacService::new();
    hvac_service
        .user_controllable_service_server
        .register_service_interface(Arc::new(user_controllable_service_impl));

    let register_settings_request = RegisterSettingsRequest {
        group_name: SETTINGS_GROUP_NAME.to_string(),
        version: String::from("1.0"),
        settings_definitions: setting_definitions().values().cloned().collect(),
        caller_fqin: SERVICE_FQIN.to_string(),
        ..Default::default()
    };
    println!(
        "Registering settings group with name: {},\nVersion: {}",
        register_settings_request.group_name, register_settings_request.version,
    );

    print_setting_definition(&register_settings_request.settings_definitions);

    let registry_service_client =
        hvac_service.get_user_preferences_registry_service_client("UserPreferencesServer");

    let response = registry_service_client
        .register_settings(register_settings_request.clone())
        .expect("Failed to perform GRPC request to register default settings");
    if response.error.is_some() {
        panic!("Failed to register settings: {:#?}", response.error.unwrap());
    }

    println!("Registering the same settings again\n\n");

    let response = registry_service_client
        .register_settings(register_settings_request)
        .expect("Failed to perform GRPC request to re-register settings");
    if response.error.is_some() {
        panic!("Failed to register settings: {:#?}", response.error.unwrap());
    }

    loop {
        std::thread::park();
    }
}
