// Copyright 2024 Google LLC

#![allow(missing_docs)]

use diagnostics_v1_sample_catalog::messages::*;
use sdv::diagnostics::api::response_code::ResponseCode;
use sdv::diagnostics::{
    data_item::*, fault_listener::*, io_control::*, routine::*, DiagnosticsResult,
};
use sdv::mw::SdvComms;
use sdv_dump::DumpBinderConnection;
use sdv_mw_rs_com_sdv_oem_sample_diagnostics_diagnostics_provider1::{
    diagnostics::Diagnostics, DiagnosticsProvider1,
};
use std::future;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use log::{debug, trace, warn};
use protobuf::well_known_types::empty::Empty;
use sdv::diagnostics::api::event::Status;
use sdv::status::{SdvStatus, SdvStatusCode};
use std::thread::JoinHandle;
use tokio_util::sync::CancellationToken;

struct PressureDataItem;
#[async_trait::async_trait]
impl ReadOnlyDataItemHandler<Pressure> for PressureDataItem {
    async fn read(&self) -> DiagnosticsResult<Pressure> {
        println!("Read Pressure DataItem called");
        Ok(Pressure { value: 100.1f32, ..Default::default() })
    }
}

struct LowPressureThresholdDataItem;
#[async_trait::async_trait]
impl DataItemHandler<Pressure> for LowPressureThresholdDataItem {
    async fn read(&self) -> DiagnosticsResult<Pressure> {
        println!("Read LowPressureThreshold DataItem called");
        Ok(Pressure { value: 100.1f32, ..Default::default() })
    }

    async fn write(&self, _: Pressure) -> ResponseCode {
        println!("Write LowPressureThreshold DataItem called: unimplemented");
        ResponseCode::GENERAL_REJECT
    }
}

struct PressurizationRoutine;
#[async_trait::async_trait]
impl RoutineHandler<Pressure, Empty, Empty, Empty, Empty, Empty> for PressurizationRoutine {
    async fn start(&self, pressure: Pressure) -> RoutineResult<Empty> {
        println!("Start PressurizationRoutine called, pressure [{}]", pressure);
        Ok(RoutineResponse::<Empty>::default())
    }
}

struct PressureIoControl;
#[async_trait::async_trait]
impl IoControlHandler<Pressure> for PressureIoControl {
    async fn return_control_to_ecu(&self, _mask: &[u8]) -> DiagnosticsResult<Pressure> {
        todo!();
    }
    async fn reset_to_default(&self, _mask: &[u8]) -> DiagnosticsResult<Pressure> {
        todo!();
    }
    async fn freeze_current_state(&self, _mask: &[u8]) -> DiagnosticsResult<Pressure> {
        println!("Freeze current state for PressureIoControl called");
        Ok(Pressure { value: 100.1f32, ..Default::default() })
    }
    async fn short_term_adjustment(
        &self,
        _adjusted_value: Pressure,
        _mask: &[u8],
    ) -> DiagnosticsResult<Pressure> {
        todo!();
    }
}

struct LowPressureFault;
#[async_trait::async_trait]
impl FaultListenerHandler<LowPressureData> for LowPressureFault {
    async fn on_fault_status_change(&self, status: u8, data: Option<LowPressureData>) {
        println!(
            "On fault status change for LowPressureFault called: status [{status}], data [{data:?}]"
        )
    }
}

static BINDER_TAG: &str = "diagnostics_test";
static DUMP_REPORT_TITLE: &str = "SDV Sample Dump - Diagnostics Test";

pub struct DiagnosticsProvider1Service {
    // Entry point for comm stack APIs.
    context: ContextRef,
    // Used to shutdown a runtime after on_stop request
    cancellation_token: Option<CancellationToken>,
    // Execution thread to get results when the bundle is stopped.
    execution_thread: Option<JoinHandle<()>>,
}

// Register the new service bundle.
sdv::lifecycle::register_service_bundle!(DiagnosticsProvider1Service);

impl ServiceBundle for DiagnosticsProvider1Service {
    /// Creates a new instance of the DiagnosticsProvider1Service.
    /// Called when service bundle is created by the system.
    ///
    /// Context object is provided as a parameter that gives access to the
    /// communication stack APIs.
    fn new(context: ContextRef) -> DiagnosticsProvider1Service {
        // Memory allocations and static data loading should be done as
        // part of this method.
        //
        // Loading of the dynamic resources (sockets, files, etc)
        // is strongly discouraged due to possible Suspend-to-RAM scenario.
        //
        // The dynamic data can be loaded in the [Self::on_start()] method.
        debug!("Creating {}.", context.get_self_fqin());
        DiagnosticsProvider1Service { context, cancellation_token: None, execution_thread: None }
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
impl Drop for DiagnosticsProvider1Service {
    fn drop(&mut self) {
        // Static resources deallocation needs to be implemented in the drop method.
        debug!("Drop {}.", self.context.get_self_fqin());
    }
}

impl DiagnosticsProvider1Service {
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
                res = diagnostics_provider1_service_main(context) => res,
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

async fn diagnostics_provider1_service_main(ctx: ContextRef) -> sdv::status::SdvResult<()> {
    sdv_dump::ProcessState::start_thread_pool();
    sdv_comms::wait_for_agents()?;

    // Implementing dump() for the sample to be able to reliably monitor it from automated tests
    let dm = DumpBinderConnection::initialize(DUMP_REPORT_TITLE, BINDER_TAG);

    let sdv_comms = SdvComms { context: ctx };
    dm.set_self_fqin(&ctx.get_self_fqin());

    let service = DiagnosticsProvider1::new(
        Arc::new(sdv_comms),
        (
            Diagnostics::create_data_item_server(
                Arc::new(PressureDataItem {}),
                Arc::new(LowPressureThresholdDataItem {}),
            ),
            Diagnostics::create_routine_control_server(Arc::new(PressurizationRoutine {})),
            Diagnostics::create_io_control_server(Arc::new(PressureIoControl {})),
            Diagnostics::create_fault_listener_server(Arc::new(LowPressureFault {})),
        ),
    )
    .await
    .unwrap();

    println!("Diagnostics provider service bundle started.");

    let result_pass = service.diagnostics.lowpressure_event_publisher.publish(Status::PASS);
    println!("{result_pass:#?}");
    thread::sleep(Duration::from_secs(1));
    let result_pre_fail = service.diagnostics.lowpressure_event_publisher.publish(Status::PRE_FAIL);
    println!("{result_pre_fail:#?}");

    if result_pass.is_ok() && result_pre_fail.is_ok() {
        dm.set_agent_state("Diagnostics Test Ok");
    } else {
        dm.set_agent_state("Diagnostics Test Error");
    }
    future::pending::<sdv::status::SdvResult<()>>().await
}
