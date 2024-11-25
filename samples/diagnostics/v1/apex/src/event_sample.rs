// Copyright 2024 Google LLC
//! Sample service exposing diagnostics data item.

use log::info;
use std::{sync::Arc, thread};
use tokio_util::sync::CancellationToken;

use sdv::mw::SdvComms;
use sdv_lifecycle_client::register_service_bundle;

use diagnostics_v1_api::event::{operation_cycle::*, *};
use diagnostics_v1_sample_catalog::messages::*;
use sdv_mw_rs_com_sdv_oem_sample_diagnostics_diagnostics_sample_with_event::publisher::Variant;
use sdv_mw_rs_com_sdv_oem_sample_diagnostics_diagnostics_sample_with_event::DiagnosticsSampleWithEvent;

register_service_bundle!(DiagnosticsSampleWithEventRunner);
struct DiagnosticsSampleWithEventRunner {
    context: ContextRef,
    cancellation_token: Option<CancellationToken>,
    background_thread: Option<std::thread::JoinHandle<()>>,
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
            let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
            rt.block_on(async {
                let comms = Arc::new(SdvComms { context });
                let mut service_bundle =
                    DiagnosticsSampleWithEvent::new(comms.clone()).await.unwrap();

                service_bundle
                    .take_publisher::<OperationCycle>(Variant::<OperationCycle>::UNIQUE)
                    .unwrap_or_else(|| panic!("Error taking publisher for OperationCycle"))
                    .publish(&OperationCycle {
                        id: "ignition".to_string(),
                        command: Command::START.into(),
                        ..Default::default()
                    })
                    .unwrap();

                service_bundle
                    .take_publisher::<EnableCondition>(Variant::<EnableCondition>::UNIQUE)
                    .unwrap_or_else(|| panic!("Error taking publisher for EnableCondition"))
                    .publish(&EnableCondition {
                        id: "freezing_temperature".to_string(),
                        enabled: true,
                        ..Default::default()
                    })
                    .unwrap();

                service_bundle
                    .diagnostics
                    .overheating_event_publisher
                    .publish_with_data(
                        Status::PRE_PASS,
                        OverheatingData { status: "too hot!".to_string(), ..Default::default() },
                    )
                    .unwrap();

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
            let _ = thread.join();
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
        while !cancellation_token.is_cancelled() {}
        info!("Service is exiting the main loop");
    }
}
