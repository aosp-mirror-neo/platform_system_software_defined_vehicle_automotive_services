// Copyright 2024 Google LLC

#![allow(missing_docs)]

use sdv::mw::SdvComms;
use sdv_mw_rs_com_sdv_oem_sample_diagnostics_diagnostics_provider2::DiagnosticsProvider2;
use std::sync::Arc;

use diagnostics_v1_sample_catalog::messages::OverheatingData;
use log::{debug, info, trace, warn};
use sdv::diagnostics::api::event::Status;
use sdv::status::{SdvStatus, SdvStatusCode};
use std::thread::JoinHandle;
use tokio_util::sync::CancellationToken;

pub struct DiagnosticsProvider2Service {
    // Entry point for comm stack APIs.
    context: ContextRef,
    // Used to shutdown a runtime after on_stop request
    cancellation_token: Option<CancellationToken>,
    // Execution thread to get results when the bundle is stopped.
    execution_thread: Option<JoinHandle<()>>,
}

// Register the new service bundle.
sdv::lifecycle::register_service_bundle!(DiagnosticsProvider2Service);

impl ServiceBundle for DiagnosticsProvider2Service {
    /// Creates a new instance of the DiagnosticsProvider2Service.
    /// Called when service bundle is created by the system.
    ///
    /// Context object is provided as a parameter that gives access to the
    /// communication stack APIs.
    fn new(context: ContextRef) -> DiagnosticsProvider2Service {
        // Memory allocations and static data loading should be done as
        // part of this method.
        //
        // Loading of the dynamic resources (sockets, files, etc)
        // is strongly discouraged due to possible Suspend-to-RAM scenario.
        //
        // The dynamic data can be loaded in the [Self::on_start()] method.
        debug!("Creating {}.", context.get_self_fqin());
        DiagnosticsProvider2Service { context, cancellation_token: None, execution_thread: None }
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
impl Drop for DiagnosticsProvider2Service {
    fn drop(&mut self) {
        // Static resources deallocation needs to be implemented in the drop method.
        debug!("Drop {}.", self.context.get_self_fqin());
    }
}

impl DiagnosticsProvider2Service {
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
                res = diagnostics_provider2_service_main(context) => res,
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

async fn diagnostics_provider2_service_main(context: ContextRef) -> sdv::status::SdvResult<()> {
    let sdv_comms = SdvComms { context };
    let service = DiagnosticsProvider2::new(Arc::new(sdv_comms)).await?;

    info!("Diagnostics provider service bundle started.");

    let result = service.diagnostics.overheating_event_publisher.publish(Status::PASS);
    info!("{result:#?}");

    let result = service
        .diagnostics
        .overheating_event_publisher
        .publish_with_data(Status::PASS, OverheatingData::default());
    info!("{result:#?}");

    loop {
        std::thread::park();
    }
}
