// Copyright 2024 Google LLC
#![allow(missing_docs)]

mod macros;
mod setting;
mod settings_group;
mod settings_group_id;
mod user_preferences_service_impl;
mod vehicle_configuration;

use sdvgenerated::user_preferences_server::UserPreferencesServer;
use std::sync::Arc;
use user_preferences_service_impl::UserPreferencesServiceImpl;

fn main() {
    let user_preferences_impl = Arc::new(UserPreferencesServiceImpl::new());
    let service = UserPreferencesServer::new();

    service
        .user_preferences_registry_service_server
        .register_service_interface(user_preferences_impl.clone());
    service
        .user_preferences_admin_service_server
        .register_service_interface(user_preferences_impl.clone());
    service
        .user_preferences_management_service_server
        .register_service_interface(user_preferences_impl.clone());

    println!("UserPreferencesServer service bundle started.");
    loop {
        std::thread::park();
    }
}
