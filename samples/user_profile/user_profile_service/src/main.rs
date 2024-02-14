// Copyright 2024 Google LLC
#![allow(missing_docs, dead_code)]

mod config_id;
mod config_metadata;
mod macros;
mod user_profile_service_impl;

use sdvgenerated::user_profile_server::UserProfileServer;
use std::sync::Arc;
use user_profile_service_impl::UserProfileServiceImpl;

fn main() {
    let user_profile_impl = Arc::new(UserProfileServiceImpl::new());
    let service = UserProfileServer::new();

    service
        .user_profile_registry_service_server
        .register_service_interface(user_profile_impl.clone());
    service.user_profile_management_service_server.register_service_interface(user_profile_impl);

    println!("UserProfileServer service bundle started.");
    loop {
        std::thread::park();
    }
}
