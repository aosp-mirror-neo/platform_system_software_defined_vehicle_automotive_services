// Copyright 2024 Google LLC
#![allow(missing_docs, unused_macros, unused_imports)]

mod macros;
mod user_preferences_service_impl;

use sdvgenerated::user_preferences_server::UserPreferencesServer;
use std::sync::Arc;
use user_preferences_service_impl::UserPreferencesServiceImpl;

fn main() {
    let user_preferences_impl = Arc::new(UserPreferencesServiceImpl::new());
    let service = UserPreferencesServer::new();

    service
        .sdv_user_preferences_user_preferences_registry_service_server
        .register_service_interface(user_preferences_impl.clone());
    service
        .sdv_user_preferences_user_preferences_admin_service_server
        .register_service_interface(user_preferences_impl.clone());
    service
        .sdv_user_preferences_user_preferences_management_service_server
        .register_service_interface(user_preferences_impl.clone());

    println!("UserPreferencesServer service bundle started.");
    loop {
        std::thread::park();
    }
}
