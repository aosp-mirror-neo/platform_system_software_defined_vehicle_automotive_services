// Copyright 2024 Google LLC
#![allow(missing_docs)]

mod properties;

use sdvgenerated::hvac_service::HvacService;
use std::sync::Arc;
use user_controllable_service_impl::{SettingGroupDefinition, SimpleUserControllableServiceImpl};

const SERVICE_FQIN: &str = "HvacService";

fn main() {
    let hvac_service = HvacService::new();

    let user_preferences_registry_service_client = hvac_service
        .get_sdv_user_preferences_user_preferences_registry_service_client("UserPreferencesServer");
    let hvac_setting_definition = SettingGroupDefinition {
        name: properties::HVAC_SETTINGS_GROUP_NAME.to_string(),
        version: String::from("1"),
        setting_definitions: vec![
            properties::hvac_enabled_definition(),
            properties::fan_speed_definition(),
            properties::temperature_definition(),
        ],
    };
    let mock_user_controllable_service_impl = Arc::new(SimpleUserControllableServiceImpl::new(
        SERVICE_FQIN.to_string(),
        vec![hvac_setting_definition],
        user_preferences_registry_service_client.clone(),
    ));

    hvac_service
        .sdv_user_preferences_user_controllable_user_controllable_service_server
        .register_service_interface(mock_user_controllable_service_impl.clone());

    println!("Started {} service bundle", SERVICE_FQIN);
    loop {
        std::thread::park();
    }
}
