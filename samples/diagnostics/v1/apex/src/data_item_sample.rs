// Copyright 2024 Google LLC
//! Sample service exposing diagnostics data item.

use log::{debug, error, info};
use std::{sync::Arc, thread};
use tokio_util::sync::CancellationToken;

use sdv::mw::SdvComms;
use sdv_lifecycle_client::register_service_bundle;

use diagnostics_v1_sample_catalog::messages::*;
use sdv::diagnostics::data_item::ReadOnlyDataItemHandler;
use sdv::diagnostics::DiagnosticsResult;
use sdv_mw_rs_com_sdv_oem_sample_diagnostics_diagnostics_sample_with_data_item::diagnostics::Diagnostics;
use sdv_mw_rs_com_sdv_oem_sample_diagnostics_diagnostics_sample_with_data_item::DiagnosticsSampleWithDataItem;

register_service_bundle!(DiagnosticsSampleWithDataItemRunner);
struct DiagnosticsSampleWithDataItemRunner {
    context: ContextRef,
    cancellation_token: Option<CancellationToken>,
    background_thread: Option<std::thread::JoinHandle<()>>,
}

struct PressureDataItem;
#[async_trait::async_trait]
impl ReadOnlyDataItemHandler<Pressure> for PressureDataItem {
    async fn read(&self) -> DiagnosticsResult<Pressure> {
        println!("Read Pressure DataItem called");
        Ok(Pressure { value: 100.1f32, ..Default::default() })
    }
}

impl ServiceBundle for DiagnosticsSampleWithDataItemRunner {
    fn new(context: ContextRef) -> DiagnosticsSampleWithDataItemRunner {
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
                let service_bundle = DiagnosticsSampleWithDataItem::new(
                    comms.clone(),
                    (Diagnostics::create_data_item_server(Arc::new(PressureDataItem {})),),
                )
                .await;

                let Ok(service_bundle) = service_bundle else {
                    error!("Failed to create service bundle");
                    return;
                };

                debug!("data_item_sample: After Service Bundle creation");

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

impl DiagnosticsSampleWithDataItemRunner {
    async fn main_loop(
        _ctx: ContextRef,
        _service_bundle: DiagnosticsSampleWithDataItem,
        cancellation_token: CancellationToken,
    ) {
        info!("Service is running in the main loop");
        while !cancellation_token.is_cancelled() {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
        info!("Service is exiting the main loop");
    }
}
