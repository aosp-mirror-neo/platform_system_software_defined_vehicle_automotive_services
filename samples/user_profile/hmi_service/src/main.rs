// Copyright 2024 Google LLC
#![allow(missing_docs)]

mod hmi_service_impl;

use std::sync::Arc;

use protobuf::{Enum, MessageField};
use sdvgenerated::hmi_service::HmiService;
use sdvgenerated::user_profile_admin_service_interface::UserProfileAdminService;
use sdvgenerated::user_profile_management_service_interface::UserProfileManagementService;
use user_profile_api::setting::setting::Value;
use user_profile_api::setting::Setting;
use user_profile_api::user_profile_admin_service::CreateUserRequest;
use user_profile_api::user_profile_admin_service::DeleteUserRequest;
use user_profile_api::user_profile_admin_service::FactoryResetRequest;
use user_profile_api::user_profile_admin_service::SelectUserRequest;
use user_profile_api::user_profile_management_service::RequestSettingsChangeRequest;
use user_profile_api::{
    user::user::UserFlags, user::User,
    user_profile_management_service::SubscribeToSettingsChangeAndGetSettingsRequest,
};

use hmi_service_impl::HmiServiceImpl;
use user_profile_utils::print_settings;

pub const SETTINGS_GROUP_NAME: &str = "HVAC";
pub const TEMPERATURE: &str = "TEMPERATURE";

fn main() {
    let hmi_service_impl = Arc::new(HmiServiceImpl::new());
    let hmi_service = HmiService::new();
    hmi_service.user_profile_view_server.register_service_interface(hmi_service_impl);

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
                flags: UserFlags::DRIVER.value() | UserFlags::EPHEMERAL.value(),
                ..Default::default()
            }),
            ..Default::default()
        })
        .expect("Failed to perform request to create user profile");
    if let Some(error) = response.error.into_option() {
        panic!("Failed to create profile with ID {}: {:?}", profile_id, error);
    } else {
        println!("Successfully created profile with ID {}\n", profile_id);
    }

    let response = user_profile_admin_client
        .select_user(SelectUserRequest { user_id: profile_id, ..Default::default() })
        .expect("Failed to perform request to select user profile");
    if let Some(error) = response.error.into_option() {
        panic!("Failed to select profile with ID {}: {:?}", profile_id, error);
    }

    // HVAC requires 3 seconds to make the change, lets wait for 5 seconds before changing the profile
    std::thread::sleep(std::time::Duration::from_secs(5));

    let response = user_profile_management_client
        .subscribe_to_settings_change_and_get_settings(
            SubscribeToSettingsChangeAndGetSettingsRequest {
                group_name: "HVAC".to_string(),
                ..Default::default()
            },
        )
        .expect("Failed to perform request to subscribe to settings changes");
    if let Some(error) = response.error.into_option() {
        panic!("Failed to subscribe to settings change: {:?}", error);
    }
    println!("Current HVAC settings");
    print_settings(&response.group.unwrap().settings);

    println!("Adjusting HVAC temperature to 32");
    let response = user_profile_management_client
        .request_settings_change(RequestSettingsChangeRequest {
            group_name: SETTINGS_GROUP_NAME.to_string(),
            settings: vec![Setting {
                key: TEMPERATURE.to_string(),
                value: Some(Value::Int64(32)),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("Failed to perform request setting change");
    if let Some(error) = response.error.into_option() {
        panic!("Failed to perform setting change {:?}", error);
    }

    // HVAC requires 3 seconds to make the change, lets wait for 5 seconds before changing the profile
    std::thread::sleep(std::time::Duration::from_secs(5));

    println!("Creating another user profile");
    let profile_id_2 = 2_u64;
    let response = user_profile_admin_client
        .create_user(CreateUserRequest {
            user: MessageField::some(User {
                id: profile_id_2,
                flags: UserFlags::DRIVER.value(),
                ..Default::default()
            }),
            ..Default::default()
        })
        .expect("Failed to perform request to create user profile");
    if let Some(error) = response.error.into_option() {
        panic!("Failed to create profile with ID {}: {:?}", profile_id_2, error);
    } else {
        println!("Successfully created profile with ID {}", profile_id_2);
    }

    let response = user_profile_admin_client
        .select_user(SelectUserRequest { user_id: profile_id_2, ..Default::default() })
        .expect("Failed to perform request to select user profile");
    if let Some(error) = response.error.into_option() {
        panic!("Failed to select profile with ID {}: {:?}", profile_id_2, error);
    }

    println!("Switched to profile {}", profile_id_2);

    // Switching profile will result in a callback for settings change, sleep until we receive the callback
    // to avoid mixing the order of the messages in the standard output
    std::thread::sleep(std::time::Duration::from_secs(5));

    let profile_id_3 = 3_u64;
    println!("Testing profile deletion, creating profile with ID {}", profile_id_3);
    let response = user_profile_admin_client
        .create_user(CreateUserRequest {
            user: MessageField::some(User {
                id: profile_id_3,
                flags: UserFlags::DRIVER.value(),
                ..Default::default()
            }),
            ..Default::default()
        })
        .expect("Failed to perform request to create user profile");
    if let Some(error) = response.error.into_option() {
        panic!("Failed to create profile with ID {}: {:?}", profile_id_3, error);
    } else {
        println!("Successfully created profile with ID {}", profile_id_3);
    }

    let response = user_profile_admin_client
        .delete_user(DeleteUserRequest { user_id: profile_id_3, ..Default::default() })
        .expect("Failed to perform request to delete profile");
    if let Some(error) = response.error.into_option() {
        panic!("Failed to delete profile with ID {}: {:?}", profile_id_3, error);
    }
    println!("Deleted user profile {}", profile_id_3);

    let response = user_profile_admin_client
        .factory_reset(FactoryResetRequest::new())
        .expect("Failed to perform request to factory reset");
    if let Some(error) = response.error.into_option() {
        panic!("Failed to perform factory reset: {:?}", error);
    }
    println!("Performed factory reset");

    loop {
        std::thread::park()
    }
}
