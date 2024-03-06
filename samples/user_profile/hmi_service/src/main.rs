// Copyright 2024 Google LLC
#![allow(missing_docs)]

use protobuf::{Enum, MessageField};
use sdvgenerated::hmi_service::HmiService;
use sdvgenerated::user_profile_admin_service_interface::UserProfileAdminService;
use sdvgenerated::user_profile_management_service_interface::UserProfileManagementService;
use user_profile_api::{
    user::user::UserFlags, user::User, user_profile_admin_service::CreateUserRequest,
    user_profile_management_service::SubscribeToConfigChangeAndGetConfigRequest,
};

fn main() {
    let hmi_service = HmiService::new();
    let user_profile_admin_client =
        hmi_service.get_user_profile_admin_service_client("UserProfileServer");
    let user_profile_management_client =
        hmi_service.get_user_profile_management_service_client("UserProfileServer");

    let profile_id = 1_u64;
    println!("Creating a user profile");
    let response = user_profile_admin_client
        .create_user(CreateUserRequest {
            user: MessageField::some(User {
                id: profile_id,
                flags: UserFlags::DRIVER.value(),
                ..Default::default()
            }),
            ..Default::default()
        })
        .expect("Failed to perform GRPC request to create user profile");
    if response.error.is_some() {
        panic!("Failed to create profile with ID {}: {:?}", profile_id, response.error.unwrap());
    } else {
        println!("Successfully created profile with ID {}", profile_id);
    }

    let response = user_profile_management_client
        .subscribe_to_config_change_and_get_config(SubscribeToConfigChangeAndGetConfigRequest {
            config_name: "HVAC".to_string(),
            ..Default::default()
        })
        .expect("Failed to perform GRPC request to subscribe to configuration changes");
    if response.error.is_some() {
        panic!("Failed to subscribe to configuration change: {:?}", response.error.unwrap());
    }

    println!("Current HVAC configuration: {:#?}", response.config.unwrap());
}
