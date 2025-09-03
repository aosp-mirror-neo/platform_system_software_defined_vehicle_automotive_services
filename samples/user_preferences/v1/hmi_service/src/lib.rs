// Copyright 2025 Google LLC
#![allow(missing_docs)]

use com_sdv_google_user_preferences_user_preferences_admin_service_rpc::Client as AdminServiceClient;
use com_sdv_google_user_preferences_user_preferences_management_service_rpc::Client as ManagementServiceClient;
use futures::future;
use futures::stream::StreamExt;
use hmi_service::HmiServiceImpl;
use log::{debug, error, info, trace, warn};
use protobuf::Enum;
use sdv::status::{SdvStatus, SdvStatusCode};
use sdv_comms::id::ServiceFqin;
use sdv_comms::sd::ServiceUnitChangeEvent;
use sdv_comms::sd::{ServiceDiscoveryManager, UnitType};
use sdv_mw_rs_com_sdv_oem_user_preferences_hmi_service::HMIService;
use sdv_mw_rs_comms::{ClientOptions, RpcClient, UnitName};
use sdv_mw_rs_sdv_comms::SdvComms;
use sdv_status_rs::SdvResult;
use std::sync::Arc;
use std::thread::JoinHandle;
use tokio_stream::StreamMap;
use tokio_util::sync::CancellationToken;
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

pub struct HmiService {
    // Entry point for comm stack APIs.
    context: ContextRef,
    // Used to shutdown a runtime after on_stop request
    cancellation_token: Option<CancellationToken>,
    // Execution thread to get results when the bundle is stopped.
    execution_thread: Option<JoinHandle<()>>,
}

// Register the new service bundle.
sdv::lifecycle::register_service_bundle!(HmiService);

impl ServiceBundle for HmiService {
    /// Creates a new instance of the HmiService.
    /// Called when service bundle is created by the system.
    ///
    /// Context object is provided as a parameter that gives access to the
    /// communication stack APIs.
    fn new(context: ContextRef) -> HmiService {
        // Memory allocations and static data loading should be done as
        // part of this method.
        //
        // Loading of the dynamic resources (sockets, files, etc)
        // is strongly discouraged due to possible Suspend-to-RAM scenario.
        //
        // The dynamic data can be loaded in the [Self::on_start()] method.
        debug!("Creating {}.", context.get_self_fqin());
        HmiService { context, cancellation_token: None, execution_thread: None }
    }

    /// Called when the service bundle is started by the system.
    fn on_start(&mut self) {
        // Dynamic resources(sockets, files, etc) should be allocated during this call.
        debug!("Starting {}.", self.context.get_self_fqin());
        // Spawn new execution thread to avoid blocking the `on_start` method.
        // New thread creates tokio runtime and starts MW and Comm Stack.
        let new_thread = spawn_execution_thread(self.create_cancellation_token(), self.context);
        self.execution_thread = Some(new_thread);
    }

    /// Called when the service bundle is stopped by the system in preparation
    /// for shutdown or suspend to RAM/Disc.
    fn on_stop(&mut self) {
        // Stop phase requires the service bundle to delete the dynamic resources
        // (sockets, files, etc) that were previously allocated in the [Self::on_start()] method.
        debug!("Stopping {}.", self.context.get_self_fqin());
        // Trigger cancellation token to exit Tokio runtime execution and drop it.
        if let Some(cancellation_token) = self.cancellation_token.as_mut() {
            cancellation_token.cancel();
            self.cancellation_token = None;
        } else {
            warn!("Cancellation token must have been created on_start.");
        }
        trace!("Joining execution thread {:#?}.", self.execution_thread);
        // TODO (b/392541025): Add waiting timeout to avoid JoinHandle::join be blocked.
        let result = self.execution_thread.take().map(JoinHandle::join);
        trace!("Joined execution thread with {result:?}.");
    }
}

/// Called when the service bundle is destroyed by the system.
impl Drop for HmiService {
    fn drop(&mut self) {
        // Static resources deallocation needs to be implemented in the drop method.
        debug!("Drop {}.", self.context.get_self_fqin());
    }
}

impl HmiService {
    fn create_cancellation_token(&mut self) -> CancellationToken {
        let cancellation_token = CancellationToken::new();
        self.cancellation_token = Some(cancellation_token.clone());
        cancellation_token
    }
}

fn spawn_execution_thread(
    cancellation_token: CancellationToken,
    context: ContextRef,
) -> JoinHandle<()> {
    let Ok(runtime) =
        tokio::runtime::Builder::new_current_thread().enable_time().enable_io().build()
    else {
        panic!("Failed to start the tokio runtime!");
    };
    std::thread::spawn(move || {
        // Due to async nature of MW/Comms APIs we have
        // to execute a service bundle inside a runtime (Tokio).
        runtime.block_on(async move {
            debug!("Starting {} execution loop", context.get_self_fqin());

            // Call `foo_main` which contains a business logic.
            // The function is not supposed to return.
            // The execution must be halted only by `on_stop` request coming from the
            // lifecycle manager.
            let res = tokio::select! {
                res = hmi_service_main(context) => res,
                () = cancellation_token.cancelled() => Err(SdvStatus::new(SdvStatusCode::Cancelled)),
            };
            debug!("Stopped {} execution loop with result: {res:?}.", context.get_self_fqin());
        });
        // Dropping tokio runtime immediately stops all the execution of a business logic
        // and gracefully free all the allocated resources.
        // Discovering of service units offered by this service bundle are no longer possible.
        //
        // This does not stop a thread created with `tokio::spawn_blocking` or `std::thread::spawn`.
        // The created execution thread will be joined in `onStop` method.
        drop(runtime);
    })
}

async fn hmi_service_main(context: ContextRef) -> SdvResult<()> {
    let sdv_coms = Arc::new(SdvComms { context });

    let hvac_service_fqin = get_hvac_service_fqin(context).await?;

    let hmi_service_impl = Arc::new(HmiServiceImpl::new());
    let service = HMIService::new(sdv_coms, (hmi_service_impl.clone(),)).await?;

    let admin_service_client: AdminServiceClient = service
        .create_rpc_client::<AdminServiceClient>(
            UnitName::builder()
                .package_name("com.sdv.oem.user_preferences")
                .bundle_name("UserPreferencesServiceBundle")
                .service_unit_name(AdminServiceClient::DEFAULT_UNIT_NAME)
                .build()?,
            ClientOptions::default(),
        )
        .await?;
    let management_service_client: ManagementServiceClient = service
        .create_rpc_client::<ManagementServiceClient>(
            UnitName::builder()
                .package_name("com.sdv.oem.user_preferences")
                .bundle_name("UserPreferencesServiceBundle")
                .service_unit_name(ManagementServiceClient::DEFAULT_UNIT_NAME)
                .build()?,
            ClientOptions::default(),
        )
        .await?;

    let user_id = 1_i32;
    info!("Creating an ephemeral user");
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
        .await?;

    admin_service_client.SelectUser(&SelectUserRequest { user_id, ..Default::default() }).await?;

    info!("Subscribing to setting changes");
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
        .await?;

    info!("Current HVAC settings");
    print_settings_and_constraints(&response.active_settings);

    info!("Adjusting HVAC temperature to 32");
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
        .await?;

    hmi_service_impl.wait_for_settings_change().await;

    info!("Attempting to Adjust HVAC temperature to invalid value 100");

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
        info!("Request to set invalid temperature denied {err}\n");
    } else {
        panic!("Expected request to set invalid temperature to fail");
    }

    info!("Creating another user");
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
        .await?;

    admin_service_client
        .SelectUser(&SelectUserRequest { user_id: user_id_2, ..Default::default() })
        .await?;

    hmi_service_impl.wait_for_settings_change().await;
    info!("Switched to user {user_id_2}");

    let response = admin_service_client.ListUsers(&ListUsersRequest::new()).await?;
    info!(
        "List of available users: \"{}\"\n",
        response.users.into_iter().map(|user| format!("{}", user.id)).collect::<Vec<_>>().join(",")
    );

    info!("Creating another user");
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
        .await?;

    admin_service_client
        .DeleteUser(&DeleteUserRequest { user_id: user_id_3, ..Default::default() })
        .await?;

    info!("Deleted user {user_id_3}");

    info!("Unsubscribing from setting changes");
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
        .await?;

    admin_service_client.FactoryReset(&FactoryResetRequest::new()).await?;

    info!("---------------------------");
    info!("Scenario 2: User Preference configuring HVAC temperature to persisted temperature");

    info!("Creating user");
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
        .await?;

    admin_service_client
        .SelectUser(&SelectUserRequest { user_id: user_id_4, ..Default::default() })
        .await?;

    info!("Set preferred temperature to 24");
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
        .await?;

    info!("Subscribing from setting changes");
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
        .await?;

    info!("Please restart the HVAC sample binary");
    info!("User Preferences will automatically detect it starting and set HVAC temperature to the user's desired temperature 24 ");

    let _ = future::pending::<anyhow::Result<()>>().await;

    Ok(())
}

async fn get_hvac_service_fqin(context: ContextRef) -> SdvResult<ServiceFqin> {
    let sd = ServiceDiscoveryManager::new(context);

    let unit_type = UnitType::builder()
        .set_sdv_package_name("com.sdv.google.user_preferences")
        .set_service_bundle_name("UserPreferencesServiceBundle")
        .set_unit_type_name("user_controllable.UserControllableService")
        .build()?;

    let mut stream_map = StreamMap::new();
    if let Ok(unit_type_stream) = sd.subscribe_service_unit_change_by_type(&unit_type).await {
        stream_map.insert(unit_type.clone(), unit_type_stream);
    } else {
        error!("Failed to get listener for availability changes for Unit Type {unit_type:?}");
        return Err(SdvStatus::with_message(
            SdvStatusCode::NotFound,
            format!("Failed to get listener for availability changes for Unit Type {unit_type:?}"),
        ));
    };

    while let Some((_, unit_change_event)) = stream_map.next().await {
        if let ServiceUnitChangeEvent::Registered(sud) = unit_change_event {
            let service_identity = sud.get_service_bundle_identity();
            let fqin = service_identity.get_fqin();
            if fqin.get_sdv_package_name() == "com.sdv.oem.user_preferences"
                && fqin.get_service_bundle_name() == "HvacService"
                && fqin.get_service_instance_name() == "default"
            {
                return Ok(fqin.clone());
            }
        }
    }

    panic!("Failed to find HVAC service");
}
