// Copyright 2024 Google LLC
#![allow(missing_docs)]

mod properties;

use sdvgenerated::car_seats_service::CarSeatsService;
use std::sync::Arc;
use user_controllable_service_impl::{SettingGroupDefinition, SimpleUserControllableServiceImpl};

const SERVICE_FQIN: &str = "CarSeatsService";

fn main() {
    let car_seats_service = CarSeatsService::new();

    let user_preferences_registry_service_client = car_seats_service
        .get_sdv_user_preferences_user_preferences_registry_service_client("UserPreferencesServer");
    let driver_seat_settings = SettingGroupDefinition {
        name: properties::DRIVER_SEAT_SETTINGS.to_string(),
        version: String::from("1"),
        setting_definitions: vec![properties::seat_position()],
    };
    let mock_user_controllable_service_impl = Arc::new(SimpleUserControllableServiceImpl::new(
        SERVICE_FQIN.to_string(),
        vec![driver_seat_settings],
        user_preferences_registry_service_client.clone(),
    ));

    car_seats_service
        .sdv_user_preferences_user_controllable_user_controllable_service_server
        .register_service_interface(mock_user_controllable_service_impl.clone());

    println!("Started {} service bundle", SERVICE_FQIN);
    loop {
        std::thread::park();
    }
}
