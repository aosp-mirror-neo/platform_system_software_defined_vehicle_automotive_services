// Copyright 2025 Google LLC
//! sample service bundle, which logs its lifecycle. Its lifecycle is controlled by a custom mode,
//! see the corresponding orchestration config file.

use diagnostics_update_sample_catalog::TirePressureSimple::TirePressureSimple;
use log::info;
use protobuf::well_known_types::empty::Empty;
use sdv::diagnostics::routine::RoutineResponse;
use sdv::diagnostics::routine::{RoutineHandler, RoutineResult};
use sdv::mw::SdvComms;
use sdv::status::SdvStatus;
use sdv_mw_rs_com_sdv_google_sample_diagnostics_update_updatable_diagnostics_bundle::{
    diagnostics::Diagnostics, UpdatableDiagnosticsBundle as MwUpdatableDiagnosticsBundle,
};
use std::future::Future;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use tokio_util::sync::CancellationToken;

// Register the new service bundle.
sdv::lifecycle::register_service_bundle!(UpdatableDiagnosticsBundle);

/// bundle struct
pub struct UpdatableDiagnosticsBundle {
    context: ContextRef,
    cancellation_token: Option<CancellationToken>,
    executor_thread_handle: Option<JoinHandle<Result<(), anyhow::Error>>>,
}

impl ServiceBundle for UpdatableDiagnosticsBundle {
    fn new(context: ContextRef) -> UpdatableDiagnosticsBundle {
        // The dynamic data can be loaded in the [Self::on_start()] method.
        UpdatableDiagnosticsBundle {
            context,
            cancellation_token: None,
            executor_thread_handle: None,
        }
    }

    /// Called when the service bundle is started by the system.
    fn on_start(&mut self) {
        info!("Hello diag_bundle_v1");
        let cancellation_token = CancellationToken::new();
        let cancellation_token_clone = cancellation_token.clone();
        let context_ref = self.context;
        self.cancellation_token = Some(cancellation_token.clone());
        self.executor_thread_handle = Some(thread::spawn(move || {
            run_blocking_with_cancellation(
                async move {
                    setup_server_and_serve_until_cancel(context_ref, cancellation_token_clone)
                        .await;
                    Ok(())
                },
                cancellation_token,
            )
        }));
    }

    /// Called when the service bundle is stopped by the system in preparation
    /// for shutdown or suspend to RAM/Disc.
    fn on_stop(&mut self) {}
}

/// Called when the service bundle is destroyed by the system.
impl Drop for UpdatableDiagnosticsBundle {
    fn drop(&mut self) {
        // Static resources deallocation needs to be implemented in the drop method.
    }
}
async fn setup_server_and_serve_until_cancel(
    context: ContextRef,
    cancellation_token: CancellationToken,
) {
    let _server = MwUpdatableDiagnosticsBundle::new(
        Arc::new(SdvComms { context }),
        (Diagnostics::create_routine_control_server(Arc::new(PressureRoutineControl {})),),
    )
    .await;
    cancellation_token.cancelled().await;
}

fn run_blocking_with_cancellation<T: Future<Output = Result<(), anyhow::Error>>>(
    future: T,
    cancellation_token: CancellationToken,
) -> Result<(), anyhow::Error> {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    rt.block_on(async move {
        let res = tokio::select! {
            _ = cancellation_token.cancelled() =>{ Err(SdvStatus::new(sdv::status::SdvStatusCode::Cancelled).into())},
            res = future => {res}
        };
        res
    })
}

struct PressureRoutineControl;
#[async_trait::async_trait]
impl RoutineHandler<Empty, TirePressureSimple, Empty, Empty, Empty, Empty>
    for PressureRoutineControl
{
    async fn start(&self, _req: Empty) -> RoutineResult<TirePressureSimple> {
        Ok(RoutineResponse {
            routine_info: 0,
            response: TirePressureSimple { value: 99.99f32, ..TirePressureSimple::default() },
        })
    }
}
