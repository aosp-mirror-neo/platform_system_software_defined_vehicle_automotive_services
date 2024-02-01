// Copyright 2024 Google LLC
#![allow(missing_docs, dead_code)]

mod config_id;
mod config_metadata;
mod user_profile_service_impl;

use sdvgenerated::user_profile_server::UserProfileServer;
use std::sync::Arc;
use user_profile_service_impl::UserProfileServiceImpl;

fn main() {
    let service = UserProfileServer::new();
    service
        .user_profile_registry_service_server
        .register_service_interface(Arc::new(UserProfileServiceImpl::new()));

    println!("UserProfileServer service bundle started.");
    loop {
        std::thread::park();
    }
}
