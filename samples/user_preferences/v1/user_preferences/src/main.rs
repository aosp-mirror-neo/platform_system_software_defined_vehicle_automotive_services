// Copyright 2024 Google LLC
//! Sample user preferences implementation
#![allow(dead_code)]

mod admin_service_impl;
mod management_service_impl;
mod persistence;
mod registry_service_impl;
mod rpc_client_factory;
mod settings_change_propagator_impl;
mod settings_retrieval_utils;
mod subscriptions_manager_impl;
mod user_controllable_service_listener;

#[tokio::main]
async fn main() {}
