// Copyright 2024 Google LLC
#![allow(missing_docs)]

use protobuf::MessageField;
use sdvgenerated::hmi_service::HmiService;
use sdvgenerated::user_profile_management_service_interface::UserProfileManagementService;
use user_profile_api::{
    configuration::ConfigId,
    user_profile_management_service::{CreateUserRequest, SubscribeToConfigChangeRequest},
};

fn main() {
    let hmi_service = HmiService::new();

    println!("Attempting to create a user profile with a reserved ID");
    let response = hmi_service
        .user_profile_management_service_client
        .create_user(CreateUserRequest { id: 0, ..Default::default() })
        .expect("Failed to perform GRPC request to create user profile");
    if response.error.is_some() {
        println!("User creation request rejected as expected: {:?}", response.error.unwrap());
    } else {
        panic!("Expected failure to create profile with ID 0 since it is a reserved ID")
    }

    let profile_id = 1_u64;
    println!("Creating a user profile");
    let response = hmi_service
        .user_profile_management_service_client
        .create_user(CreateUserRequest { id: profile_id, ..Default::default() })
        .expect("Failed to perform GRPC request to create user profile");
    if response.error.is_some() {
        panic!("Failed to create profile with ID {}: {:?}", profile_id, response.error.unwrap());
    } else {
        println!("Successfully created profile with ID {}", profile_id);
    }

    let response = hmi_service
        .user_profile_management_service_client
        .subscribe_to_config_change(SubscribeToConfigChangeRequest {
            config_id: MessageField::some(ConfigId {
                service_fqdn: "".to_string(),
                config_name: "HVAC".to_string(),
                ..Default::default()
            }),
            ..Default::default()
        })
        .expect("Failed to perform GRPC request to subscribe to configuration changes");
    if response.error.is_some() {
        panic!("Failed to subscribe to configuration change: {:?}", response.error.unwrap());
    }

    println!("Current HVAC configuration: {:#?}", response.config.unwrap());
}
