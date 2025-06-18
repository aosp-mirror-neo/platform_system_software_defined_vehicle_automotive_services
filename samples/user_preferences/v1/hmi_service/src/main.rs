// Copyright 2024 Google LLC
#![allow(missing_docs)]

use com_sdv_google_user_preferences_user_preferences_admin_service_rpc::Client as AdminServiceClient;
use com_sdv_google_user_preferences_user_preferences_management_service_rpc::Client as ManagementServiceClient;
use futures::stream::StreamExt;
use hmi_service::HmiServiceImpl;
use protobuf::Enum;
use sdv_comms::id::ServiceFqin;
use sdv_comms::sd::{ServiceDiscoveryManager, UnitType};
use sdv_comms::ContextRef;
use sdv_mw_rs_com_sdv_oem_user_preferences_hmi_service::HMIService;
use sdv_mw_rs_comms::{ClientOptions, RpcClient, UnitName};
use sdv_mw_rs_sdv_comms::SdvComms;
use sdv_status_rs::SdvResult;
use std::sync::Arc;
use user_preferences_api::setting::setting::Value;
use user_preferences_api::user::{user::UserFlags, User};
use user_preferences_api::{
    setting::Setting,
    settings_group::SettingsGroupId,
    user_preferences_admin_service::{
        CreateUserRequest, DeleteUserRequest, FactoryResetRequest, ListUsersRequest,
        SelectUserRequest,
    },
    user_preferences_management_service::{
        RequestSettingsChangeRequest, SubscribeToSettingsChangeAndGetSettingsRequest,
        UnsubscribeToSettingsChangeRequest,
    },
};
use user_preferences_utils::print_settings_and_constraints;
mod hmi_service;

pub const SETTINGS_GROUP_NAME: &str = "HVAC";
pub const TEMPERATURE: &str = "TEMPERATURE";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let context = create_context()?;
    let sdv_coms = Arc::new(SdvComms { context });

    let hvac_service_fqin = get_hvac_service_fqin(context).await?;

    let hmi_service_impl = Arc::new(HmiServiceImpl::new());
    let service = HMIService::new(sdv_coms, (hmi_service_impl.clone(),))
        .await
        .map_err(|e| anyhow::Error::msg(format!("Failed to start service bundle: {e}")))?;

    let admin_service_client: AdminServiceClient = service
        .create_rpc_client::<AdminServiceClient>(
            UnitName::builder()
                .package_name("com.sdv.oem.user_preferences")
                .bundle_name("UserPreferencesServiceBundle")
                .service_unit_name(AdminServiceClient::DEFAULT_UNIT_NAME)
                .build()?,
            ClientOptions::default(),
        )
        .await
        .map_err(|e| {
            anyhow::Error::msg(format!("Failed to create client to User Preferences: {e}"))
        })?;
    let management_service_client: ManagementServiceClient = service
        .create_rpc_client::<ManagementServiceClient>(
            UnitName::builder()
                .package_name("com.sdv.oem.user_preferences")
                .bundle_name("UserPreferencesServiceBundle")
                .service_unit_name(ManagementServiceClient::DEFAULT_UNIT_NAME)
                .build()?,
            ClientOptions::default(),
        )
        .await
        .map_err(|e| {
            anyhow::Error::msg(format!("Failed to create client to User Preferences: {e}"))
        })?;

    let user_id = 1_i32;
    println!("Creating an ephemeral user");
    admin_service_client
        .CreateUser(&CreateUserRequest {
            user: Some(User {
                id: user_id,
                flags: UserFlags::DRIVER.value() | UserFlags::EPHEMERAL.value(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
        .await
        .map_err(|e| anyhow::Error::msg(format!("Failed to create user: {e}")))?;

    admin_service_client
        .SelectUser(&SelectUserRequest { user_id, ..Default::default() })
        .await
        .map_err(|e| anyhow::Error::msg(format!("Failed to select user: {e}")))?;

    println!("Subscribing to setting changes");
    let response = management_service_client
        .SubscribeToSettingsChangeAndGetSettings(&SubscribeToSettingsChangeAndGetSettingsRequest {
            settings_group_id: Some(SettingsGroupId {
                service_fqin: hvac_service_fqin.to_string(),
                name: SETTINGS_GROUP_NAME.to_string(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
        .await
        .map_err(|e| anyhow::Error::msg(format!("Failed to subscribe to setting changes: {e}")))?;

    println!("Current HVAC settings");
    print_settings_and_constraints(&response.active_settings);

    println!("Adjusting HVAC temperature to 32");
    management_service_client
        .RequestSettingsChange(&RequestSettingsChangeRequest {
            settings_group_id: Some(SettingsGroupId {
                service_fqin: hvac_service_fqin.to_string(),
                name: SETTINGS_GROUP_NAME.to_string(),
                ..Default::default()
            })
            .into(),
            settings: vec![Setting {
                key: TEMPERATURE.to_string(),
                value: Some(Value::Int64(32)),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .map_err(|e| anyhow::Error::msg(format!("Failed to request setting change: {e}")))?;

    hmi_service_impl.wait_for_settings_change().await;

    println!("Attempting to Adjust HVAC temperature to invalid value 100");

    if let Err(err) = management_service_client
        .RequestSettingsChange(&RequestSettingsChangeRequest {
            settings_group_id: Some(SettingsGroupId {
                service_fqin: hvac_service_fqin.to_string(),
                name: SETTINGS_GROUP_NAME.to_string(),
                ..Default::default()
            })
            .into(),
            settings: vec![Setting {
                key: TEMPERATURE.to_string(),
                value: Some(Value::Int64(100)),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
    {
        println!("Request to set invalid temperature denied {}\n", err);
    } else {
        panic!("Expected request to set invalid temperature to fail");
    }

    println!("Creating another user");
    let user_id_2 = 2_i32;
    admin_service_client
        .CreateUser(&CreateUserRequest {
            user: Some(User {
                id: user_id_2,
                flags: UserFlags::DRIVER.value(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
        .await
        .map_err(|e| anyhow::Error::msg(format!("Failed to create user: {e}")))?;

    admin_service_client
        .SelectUser(&SelectUserRequest { user_id: user_id_2, ..Default::default() })
        .await
        .map_err(|e| anyhow::Error::msg(format!("Failed to select user: {e}")))?;

    hmi_service_impl.wait_for_settings_change().await;
    println!("Switched to user {}", user_id_2);

    let response = admin_service_client
        .ListUsers(&ListUsersRequest::new())
        .await
        .map_err(|e| anyhow::Error::msg(format!("Failed to list users: {e}")))?;
    println!(
        "List of available users: \"{}\"\n",
        response.users.into_iter().map(|user| format!("{}", user.id)).collect::<Vec<_>>().join(",")
    );

    println!("Creating another user");
    let user_id_3 = 3_i32;
    admin_service_client
        .CreateUser(&CreateUserRequest {
            user: Some(User {
                id: user_id_3,
                flags: UserFlags::DRIVER.value(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
        .await
        .map_err(|e| anyhow::Error::msg(format!("Failed to create user: {e}")))?;

    admin_service_client
        .DeleteUser(&DeleteUserRequest { user_id: user_id_3, ..Default::default() })
        .await
        .map_err(|e| {
            anyhow::Error::msg(format!("Failed to perform request to delete user: {e}"))
        })?;

    println!("Deleted user {}", user_id_3);

    println!("Unsubscribing from setting changes");
    management_service_client
        .UnsubscribeFromSettingsChange(&UnsubscribeToSettingsChangeRequest {
            settings_group_id: Some(SettingsGroupId {
                service_fqin: hvac_service_fqin.to_string(),
                name: SETTINGS_GROUP_NAME.to_string(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
        .await
        .map_err(|e| anyhow::Error::msg(format!("Failed to subscribe to setting changes: {e}")))?;

    admin_service_client
        .FactoryReset(&FactoryResetRequest::new())
        .await
        .map_err(|e| anyhow::Error::msg(format!("Failed to perform factory reset: {e}")))?;

    println!("---------------------------");
    println!("Scenario 2: User Preference configuring HVAC temperature to persisted temperature");

    println!("Creating user");
    let user_id_4 = 4_i32;
    admin_service_client
        .CreateUser(&CreateUserRequest {
            user: Some(User {
                id: user_id_4,
                flags: UserFlags::DRIVER.value(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
        .await
        .map_err(|e| anyhow::Error::msg(format!("Failed to create user: {e}")))?;

    admin_service_client
        .SelectUser(&SelectUserRequest { user_id: user_id_4, ..Default::default() })
        .await
        .map_err(|e| anyhow::Error::msg(format!("Failed to select user: {e}")))?;

    println!("Set preferred temperature to 24");
    let _response = management_service_client
        .RequestSettingsChange(&RequestSettingsChangeRequest {
            settings_group_id: Some(SettingsGroupId {
                service_fqin: hvac_service_fqin.to_string(),
                name: SETTINGS_GROUP_NAME.to_string(),
                ..Default::default()
            })
            .into(),
            settings: vec![Setting {
                key: TEMPERATURE.to_string(),
                value: Some(Value::Int64(24)),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .map_err(|e| anyhow::Error::msg(format!("Failed to request setting change: {e}")))?;

    println!("Subscribing from setting changes");
    let _response = management_service_client
        .SubscribeToSettingsChangeAndGetSettings(&SubscribeToSettingsChangeAndGetSettingsRequest {
            settings_group_id: Some(SettingsGroupId {
                service_fqin: hvac_service_fqin.to_string(),
                name: SETTINGS_GROUP_NAME.to_string(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
        .await
        .map_err(|e| anyhow::Error::msg(format!("Failed to subscribe to setting changes: {e}")))?;

    println!("Please restart the HVAC sample binary");
    println!("User Preferences will automatically detect it starting and set HVAC temperature to the user's desired temperature 24 ");
    std::thread::park();

    Ok(())
}

fn create_context() -> SdvResult<ContextRef> {
    let fqin = ServiceFqin::builder()
        .sdv_vm_name("vm")
        .sdv_package_name("com.sdv.oem.user_preferences")
        .service_bundle_name("HmiService")
        .service_instance_name("default")
        .build()?;
    Ok(ContextRef::create(fqin))
}

async fn get_hvac_service_fqin(context: ContextRef) -> anyhow::Result<ServiceFqin> {
    let sd = ServiceDiscoveryManager::new(context);

    let unit_type = UnitType::builder()
        .set_sdv_package_name("com.sdv.google.user_preferences")
        .set_service_bundle_name("UserPreferencesServiceBundle")
        .set_unit_type_name("user_controllable.UserControllableService")
        .build()
        .map_err(|e| {
            anyhow::Error::msg(format!(
                "Failed to construct unit type for user controllable services: {e}"
            ))
        })?;

    let mut service_units_stream = sd.list_units_by_type(&unit_type);

    while let Some(service_unit) = service_units_stream.next().await {
        let service_identity = service_unit.get_service_bundle_identity();
        let fqin = service_identity.get_fqin();
        if fqin.get_sdv_package_name() == "com.sdv.oem.user_preferences"
            && fqin.get_service_bundle_name() == "HvacService"
            && fqin.get_service_instance_name() == "default"
        {
            return Ok(fqin.clone());
        }
    }
    panic!("Failed to find HVAC service");
}
