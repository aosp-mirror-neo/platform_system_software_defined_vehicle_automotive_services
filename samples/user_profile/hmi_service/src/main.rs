// Copyright 2024 Google LLC
#![allow(missing_docs)]

use protobuf::{Enum, MessageField};
use sdvgenerated::hmi_service::HmiService;
use sdvgenerated::user_profile_admin_service_interface::UserProfileAdminService;
use sdvgenerated::user_profile_management_service_interface::UserProfileManagementService;
use user_profile_api::setting::setting::Value;
use user_profile_api::setting::Setting;
use user_profile_api::user_profile_admin_service::CreateUserRequest;
use user_profile_api::user_profile_admin_service::SelectUserRequest;
use user_profile_api::user_profile_management_service::RequestConfigChangeRequest;
use user_profile_api::{
    user::user::UserFlags, user::User,
    user_profile_management_service::SubscribeToConfigChangeAndGetConfigRequest,
};

use user_profile_utils::print_settings;

pub const CONFIG_NAME: &str = "HVAC";
pub const TEMPERATURE: &str = "TEMPERATURE";

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
        println!("Successfully created profile with ID {}\n", profile_id);
    }

    let response = user_profile_admin_client
        .select_user(SelectUserRequest { user_id: profile_id, ..Default::default() })
        .expect("Failed to perform GRPC request to select user profile");
    if response.error.is_some() {
        panic!("Failed to select profile with ID {}: {:?}", profile_id, response.error.unwrap());
    }

    // HVAC requires 3 seconds to make the change, lets wait for 5 seconds before changing the profile
    std::thread::sleep(std::time::Duration::from_secs(5));

    let response = user_profile_management_client
        .request_config_change(RequestConfigChangeRequest {
            config_name: CONFIG_NAME.to_string(),
            settings: vec![Setting {
                key: TEMPERATURE.to_string(),
                value: Some(Value::Int64(32)),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("Failed to perform GRPC request to request config change");
    if response.error.is_some() {
        panic!("Failed to perform config change {:?}", response.error.unwrap());
    }

    // HVAC requires 3 seconds to make the change, lets wait for 5 seconds before changing the profile
    std::thread::sleep(std::time::Duration::from_secs(5));

    let response = user_profile_management_client
        .subscribe_to_config_change_and_get_config(SubscribeToConfigChangeAndGetConfigRequest {
            config_name: "HVAC".to_string(),
            ..Default::default()
        })
        .expect("Failed to perform GRPC request to subscribe to configuration changes");
    if response.error.is_some() {
        panic!("Failed to subscribe to configuration change: {:?}", response.error.unwrap());
    }
    println!("Current HVAC configuration");
    print_settings(&response.config.unwrap().settings);

    println!("Creating another user profile");
    let profile_id_2 = 2;
    let response = user_profile_admin_client
        .create_user(CreateUserRequest {
            user: MessageField::some(User {
                id: profile_id_2,
                flags: UserFlags::DRIVER.value(),
                ..Default::default()
            }),
            ..Default::default()
        })
        .expect("Failed to perform GRPC request to create user profile");
    if response.error.is_some() {
        panic!("Failed to create profile with ID {}: {:?}", profile_id_2, response.error.unwrap());
    } else {
        println!("Successfully created profile with ID {}", profile_id_2);
    }

    let response = user_profile_admin_client
        .select_user(SelectUserRequest { user_id: profile_id_2, ..Default::default() })
        .expect("Failed to perform GRPC request to select user profile");
    if response.error.is_some() {
        panic!("Failed to select profile with ID {}: {:?}", profile_id_2, response.error.unwrap());
    }

    println!("Switched to profile {}", profile_id_2);
}
