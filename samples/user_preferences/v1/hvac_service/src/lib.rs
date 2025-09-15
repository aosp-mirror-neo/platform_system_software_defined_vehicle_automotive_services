// Copyright 2025 Google LLC
#![allow(missing_docs)]

use com_sdv_google_user_preferences_user_preferences_registry_service_rpc::Client as RegistryClient;
use futures::future;
use log::{debug, info, trace, warn};
use sdv::status::{SdvStatus, SdvStatusCode};
use sdv_mw_rs_com_sdv_oem_user_preferences_hvac_service::HVACService;
use sdv_mw_rs_comms::Communicate;
use sdv_mw_rs_comms::RpcClient;
use sdv_mw_rs_sdv_comms::SdvComms;
use std::sync::Arc;
use std::thread::JoinHandle;
use tokio_util::sync::CancellationToken;
use user_controllable_service_impl::{SettingGroupDefinition, SimpleUserControllableServiceImpl};

mod properties;

pub struct HvacService {
    // Entry point for comm stack APIs.
    context: ContextRef,
    // Used to shutdown a runtime after on_stop request
    cancellation_token: Option<CancellationToken>,
    // Execution thread to get results when the bundle is stopped.
    execution_thread: Option<JoinHandle<()>>,
}

// Register the new service bundle.
sdv::lifecycle::register_service_bundle!(HvacService);

impl ServiceBundle for HvacService {
    /// Creates a new instance of the HvacService.
    /// Called when service bundle is created by the system.
    ///
    /// Context object is provided as a parameter that gives access to the
    /// communication stack APIs.
    fn new(context: ContextRef) -> HvacService {
        // Memory allocations and static data loading should be done as
        // part of this method.
        //
        // Loading of the dynamic resources (sockets, files, etc)
        // is strongly discouraged due to possible Suspend-to-RAM scenario.
        //
        // The dynamic data can be loaded in the [Self::on_start()] method.
        debug!("Creating {}.", context.get_self_fqin());
        HvacService { context, cancellation_token: None, execution_thread: None }
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
impl Drop for HvacService {
    fn drop(&mut self) {
        // Static resources deallocation needs to be implemented in the drop method.
        debug!("Drop {}.", self.context.get_self_fqin());
    }
}

impl HvacService {
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
                res = hvac_service_main(context) => res,
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

async fn hvac_service_main(context: ContextRef) -> sdv::status::SdvResult<()> {
    let comms = Arc::new(SdvComms { context });

    let hvac_setting_definition = SettingGroupDefinition {
        name: properties::HVAC_SETTINGS_GROUP_NAME.to_string(),
        version: String::from("1"),
        setting_definitions: vec![
            properties::hvac_enabled_definition(),
            properties::fan_speed_definition(),
            properties::temperature_definition(),
        ],
    };

    let attributes = sdv_mw_rs_comms::ClientAttributes::builder()
        .server_name(
            sdv_mw_rs_comms::UnitName::builder()
                .vm_name("")
                .package_name("com.sdv.oem.user_preferences")
                .bundle_name("UserPreferencesServiceBundle")
                .service_unit_name(RegistryClient::DEFAULT_UNIT_NAME)
                .build()?,
        )
        .client_options(sdv_mw_rs_comms::ClientOptions::default())
        .build()?;

    let client = Arc::new(RegistryClient::new(comms.create_client_transport(&attributes).await?));

    let sb_implementation = Arc::new(
        SimpleUserControllableServiceImpl::new(vec![hvac_setting_definition], client).await?,
    );

    let _hvac_service = HVACService::new(comms, (sb_implementation,)).await?;

    info!("Started service bundle");
    let _ = future::pending::<anyhow::Result<()>>().await;

    Ok(())
}
