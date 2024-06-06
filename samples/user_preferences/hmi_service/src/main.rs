// Copyright 2024 Google LLC
#![allow(missing_docs)]

mod hmi_service_impl;

use std::sync::Arc;

use itertools::Itertools;
use protobuf::{Enum, MessageField};
use sdvgenerated::hmi_service::HmiService;
use sdvgenerated::sdv_user_preferences_user_preferences_admin_service_interface::UserPreferencesAdminService;
use sdvgenerated::sdv_user_preferences_user_preferences_management_service_interface::UserPreferencesManagementService;
use user_preferences_api::setting::{setting::Value, Setting};
use user_preferences_api::settings_group::SettingsGroupId;
use user_preferences_api::user::{user::UserFlags, User};
use user_preferences_api::user_preferences_admin_service::{
    CreateUserRequest, DeleteUserRequest, FactoryResetRequest, GetUserSettingsRequest,
    ListUsersRequest, SelectUserRequest,
};
use user_preferences_api::user_preferences_management_service::{
    RequestSettingsChangeRequest, SubscribeToSettingsChangeAndGetSettingsRequest,
    UnsubscribeToSettingsChangeRequest,
};

use hmi_service_impl::HmiServiceImpl;
use user_preferences_utils::print_settings;
use user_preferences_utils::print_settings_and_constraints;

pub(crate) const HVAC_SERVICE_FQIN: &str = "HvacService";
pub const SETTINGS_GROUP_NAME: &str = "HVAC";
pub const TEMPERATURE: &str = "TEMPERATURE";

fn main() {
    let hmi_service_impl = Arc::new(HmiServiceImpl::new());
    let hmi_service = HmiService::new();
    hmi_service
        .sdv_user_preferences_view_change_notifier_server
        .register_service_interface(hmi_service_impl);

    let user_preferences_admin_client = hmi_service
        .get_sdv_user_preferences_user_preferences_admin_service_client("UserPreferencesServer");
    let user_preferences_management_client = hmi_service
        .get_sdv_user_preferences_user_preferences_management_service_client(
            "UserPreferencesServer",
        );

    let user_id = 1_i32;
    println!("Creating an ephemeral user");
    let response = user_preferences_admin_client
        .create_user(CreateUserRequest {
            user: MessageField::some(User {
                id: user_id,
                flags: UserFlags::DRIVER.value() | UserFlags::EPHEMERAL.value(),
                ..Default::default()
            }),
            ..Default::default()
        })
        .expect("Failed to perform request to create user");
    if let Some(error) = response.error.into_option() {
        panic!("Failed to create user {}: {:?}", user_id, error);
    } else {
        println!("Successfully created user {}\n", user_id);
    }

    let response = user_preferences_admin_client
        .select_user(SelectUserRequest { user_id, ..Default::default() })
        .expect("Failed to perform request to select user");
    if let Some(error) = response.error.into_option() {
        panic!("Failed to select user {}: {:?}", user_id, error);
    }

    // HVAC requires 3 seconds to make the change, lets wait for 5 seconds before changing the user
    std::thread::sleep(std::time::Duration::from_secs(5));

    let response = user_preferences_management_client
        .subscribe_to_settings_change_and_get_settings(
            SubscribeToSettingsChangeAndGetSettingsRequest {
                settings_group_id: MessageField::some(SettingsGroupId {
                    service_fqin: HVAC_SERVICE_FQIN.to_string(),
                    name: SETTINGS_GROUP_NAME.to_string(),
                    ..Default::default()
                }),
                caller_fqin: String::from("HmiService"),
                ..Default::default()
            },
        )
        .expect("Failed to perform request to subscribe to settings changes");
    if let Some(error) = response.error.into_option() {
        panic!("Failed to subscribe to settings change: {:?}", error);
    }
    println!("Current HVAC settings");
    print_settings_and_constraints(&response.current_settings);

    println!("Adjusting HVAC temperature to 32");
    let response = user_preferences_management_client
        .request_settings_change(RequestSettingsChangeRequest {
            settings_group_id: MessageField::some(SettingsGroupId {
                service_fqin: HVAC_SERVICE_FQIN.to_string(),
                name: SETTINGS_GROUP_NAME.to_string(),
                ..Default::default()
            }),
            settings: vec![Setting {
                key: TEMPERATURE.to_string(),
                value: Some(Value::Int64(32)),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("Failed to request setting change");
    if let Some(error) = response.error.into_option() {
        panic!("Failed to perform setting change {:?}", error);
    }

    // HVAC requires 3 seconds to make the change, lets wait for 5 seconds before changing the user
    std::thread::sleep(std::time::Duration::from_secs(5));

    println!("Attempting to Adjust HVAC temperature to invalid value 100");
    let response = user_preferences_management_client
        .request_settings_change(RequestSettingsChangeRequest {
            settings_group_id: MessageField::some(SettingsGroupId {
                service_fqin: HVAC_SERVICE_FQIN.to_string(),
                name: SETTINGS_GROUP_NAME.to_string(),
                ..Default::default()
            }),
            settings: vec![Setting {
                key: TEMPERATURE.to_string(),
                value: Some(Value::Int64(100)),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("Failed to request setting change");
    if let Some(error) = response.error.into_option() {
        println!("Received error {:?}\n", error.message);
    } else {
        panic!("Expected request to fail");
    }

    println!("Creating another user");
    let user_id_2 = 2_i32;
    let response = user_preferences_admin_client
        .create_user(CreateUserRequest {
            user: MessageField::some(User {
                id: user_id_2,
                flags: UserFlags::DRIVER.value(),
                ..Default::default()
            }),
            ..Default::default()
        })
        .expect("Failed to perform request to create user");
    if let Some(error) = response.error.into_option() {
        panic!("Failed to create user {}: {:?}", user_id_2, error);
    } else {
        println!("Successfully created user {}", user_id_2);
    }

    let response = user_preferences_admin_client
        .select_user(SelectUserRequest { user_id: user_id_2, ..Default::default() })
        .expect("Failed to perform request to select user");
    if let Some(error) = response.error.into_option() {
        panic!("Failed to select user {}: {:?}", user_id_2, error);
    }
    println!("Switched to user {}", user_id_2);

    let response = user_preferences_admin_client
        .list_users(ListUsersRequest::new())
        .expect("Failed to perform request to list users");
    if let Some(error) = response.error.into_option() {
        panic!("Failed to list users: {}", error.message);
    }
    println!(
        "List of available users: \"{}\"\n",
        response.users.into_iter().map(|user| format!("{}", user.id)).join(",")
    );

    // Switching user will result in a callback for settings change, sleep until we receive the callback
    // to avoid mixing the order of the messages in the standard output
    std::thread::sleep(std::time::Duration::from_secs(5));

    println!("Retrieving settings for user {}", user_id_2);
    let response = user_preferences_admin_client
        .get_user_settings(GetUserSettingsRequest { user_id: user_id_2, ..Default::default() })
        .expect("Failed to perform request to get user settings");
    if let Some(error) = response.error.into_option() {
        panic!("Failed to retrieve user settings: {}", error.message);
    }

    for settings_group in response.groups {
        println!("Settings Group: {}", settings_group.id.name);
        print_settings(&settings_group.settings);
    }

    let user_id_3 = 3_i32;
    println!("Testing user deletion, creating user {}", user_id_3);
    let response = user_preferences_admin_client
        .create_user(CreateUserRequest {
            user: MessageField::some(User {
                id: user_id_3,
                flags: UserFlags::DRIVER.value(),
                ..Default::default()
            }),
            ..Default::default()
        })
        .expect("Failed to perform request to create user");
    if let Some(error) = response.error.into_option() {
        panic!("Failed to create user {}: {:?}", user_id_3, error);
    } else {
        println!("Successfully created user {}", user_id_3);
    }

    let response = user_preferences_admin_client
        .delete_user(DeleteUserRequest { user_id: user_id_3, ..Default::default() })
        .expect("Failed to perform request to delete user");
    if let Some(error) = response.error.into_option() {
        panic!("Failed to delete user {}: {:?}", user_id_3, error);
    }
    println!("Deleted user {}", user_id_3);

    println!("Unsubscribing from setting changes");
    let response = user_preferences_management_client
        .unsubscribe_from_settings_change(UnsubscribeToSettingsChangeRequest {
            settings_group_id: MessageField::some(SettingsGroupId {
                service_fqin: HVAC_SERVICE_FQIN.to_string(),
                name: SETTINGS_GROUP_NAME.to_string(),
                ..Default::default()
            }),
            caller_fqin: String::from("HmiService"),
            ..Default::default()
        })
        .expect("Failed to perform request to unsubscribe from settings changes");
    if let Some(error) = response.error.into_option() {
        panic!("Failed to unsubscribe from settings group: {:?}", error);
    }

    println!("Performing factory reset");
    let response = user_preferences_admin_client
        .factory_reset(FactoryResetRequest::new())
        .expect("Failed to perform request to factory reset");
    if let Some(error) = response.error.into_option() {
        panic!("Failed to perform factory reset: {:?}", error);
    }

    loop {
        std::thread::park()
    }
}
