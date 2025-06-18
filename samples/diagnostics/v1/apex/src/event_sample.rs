// Copyright 2024 Google LLC
//! Sample service exposing diagnostics data item.

use log::{error, info};
use std::{sync::Arc, thread};
use tokio_util::sync::CancellationToken;

use sdv::mw::SdvComms;
use sdv_lifecycle_client::register_service_bundle;

use diagnostics_v1_api::event::{operation_cycle::*, *};
use diagnostics_v1_sample_catalog::messages::*;
use sdv::diagnostics::fault_listener::FaultListenerHandler;
use sdv_mw_rs_com_sdv_oem_sample_diagnostics_diagnostics_sample_with_event::{
    diagnostics::Diagnostics, publisher::Variant, DiagnosticsSampleWithEvent,
};

register_service_bundle!(DiagnosticsSampleWithEventRunner);
struct DiagnosticsSampleWithEventRunner {
    context: ContextRef,
    cancellation_token: Option<CancellationToken>,
    background_thread: Option<std::thread::JoinHandle<()>>,
}

struct OverheatingFaultListener;
#[async_trait::async_trait]
impl FaultListenerHandler<OverheatingData> for OverheatingFaultListener {
    async fn on_fault_status_change(&self, status: u8, data: Option<OverheatingData>) {
        info!("Overheating fault status changed: [status: {status}] {data:?}");
    }
}

impl ServiceBundle for DiagnosticsSampleWithEventRunner {
    fn new(context: ContextRef) -> DiagnosticsSampleWithEventRunner {
        Self { context, cancellation_token: None, background_thread: None }
    }

    fn on_start(&mut self) {
        info!("Service is starting");

        // Create a cancellation token and pass it down the thread to a runtime
        let cancellation_token = CancellationToken::new();
        self.cancellation_token = Some(cancellation_token.clone());
        let context = self.context;

        self.background_thread = Some(thread::spawn(move || {
            let Ok(rt) = tokio::runtime::Builder::new_multi_thread().enable_all().build() else {
                error!("Failed to build Tokio runtime");
                return;
            };

            rt.block_on(async {
                let comms = Arc::new(SdvComms { context });
                let service_bundle = DiagnosticsSampleWithEvent::new(
                    comms.clone(),
                    (Diagnostics::create_fault_listener_server(Arc::new(
                        OverheatingFaultListener {},
                    )),),
                )
                .await;

                let Ok(mut service_bundle) = service_bundle else {
                    error!("Failed to create service bundle");
                    return;
                };

                // Publisher "taken" from service_bundle should be kept in memory, in order to
                // keep publication "alive".
                // Here, publications die as soon as publishers get out of scope.
                let operation_cycle_publisher = service_bundle
                    .take_publisher::<OperationCycle>(Variant::<OperationCycle>::UNIQUE)
                    .inspect(|publisher| {
                        publisher
                            .publish(&OperationCycle {
                                id: "ignition".to_string(),
                                command: Command::START.into(),
                                ..Default::default()
                            })
                            .unwrap_or_else(|e| error!("Error publishing `ignition == START` for OperationCycle: {e:#}"));
                    });

                if operation_cycle_publisher.is_none() {
                    error!("Error taking publisher for OperationCycle");
                }

                let enable_condition_publisher = service_bundle
                    .take_publisher::<EnableCondition>(Variant::<EnableCondition>::UNIQUE)
                    .inspect(|publisher| {
                        publisher.publish(&EnableCondition {
                            id: "freezing_temperature".to_string(),
                            enabled: true,
                            ..Default::default()
                        })
                        .unwrap_or_else(|e| error!("Error publishing `freezing_temperature == true` for EnableCondition: {e:#}"));
                    });

                if enable_condition_publisher.is_none() {
                    error!("Error taking publisher for EnableCondition");
                }

                service_bundle
                    .diagnostics
                    .overheating_event_publisher
                    .publish_with_data(
                        Status::PRE_PASS,
                        OverheatingData { status: "too hot!".to_string(), ..Default::default() },
                    )
                    .unwrap_or_else(|e| error!("Error publishing `too hot!` for OverheatingEvent: {e:#}"));

                Self::main_loop(context, service_bundle, cancellation_token.clone()).await;
            });
        }));

        info!("Service is running");
    }

    fn on_stop(&mut self) {
        info!("Service is stopping");

        // Trigger cancellation token to exit Tokio runtime execution and drop it
        self.cancellation_token.as_mut().expect("Must have been created on_start").cancel();

        if let Some(thread) = self.background_thread.take() {
            if thread.join().is_err() {
                error!("Failed to join background thread in on_stop");
            }
        }

        self.cancellation_token = None;

        info!("Service stopped");
    }
}

impl DiagnosticsSampleWithEventRunner {
    async fn main_loop(
        _ctx: ContextRef,
        _service_bundle: DiagnosticsSampleWithEvent,
        cancellation_token: CancellationToken,
    ) {
        info!("Service is running in the main loop");
        while !cancellation_token.is_cancelled() {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
        info!("Service is exiting the main loop");
    }
}
