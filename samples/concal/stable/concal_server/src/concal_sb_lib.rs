// Copyright 2024 Google LLC

//! SDV ConCal Server Sample Library to be used by the Lifecycle Manager Runner.

#[cfg(test)]
mod tests;

mod concal_impl;
mod configuration;
mod hashable_config_id;
mod message_editor;
mod trie;
mod versioned_object;

use crate::concal_impl::ConCalServiceBundleImpl;

use log::info;
use sdv_mw_rs_sdv_comms::SdvComms;
use sdv_mw_rs_sdv_concal_con_cal_service_bundle::ConCalServiceBundle as ConCalServiceBundleMw;
use std::time::Duration;
use std::{sync::Arc, thread};
use tokio_util::sync::CancellationToken;

/// Service bundle struct definition.
pub struct ConCalServiceBundle {
    // Entry point for comm stack APIs
    context: sdv_comms::ContextRef,
    // Used to shutdown a runtime after on_stop request
    cancellation_token: Option<CancellationToken>,
    background_thread: Option<std::thread::JoinHandle<()>>,
}

impl ConCalServiceBundle {
    async fn main_loop(cancellation_token: CancellationToken) {
        while !cancellation_token.is_cancelled() {
            thread::sleep(Duration::from_millis(1000));
        }
    }
}

// Register the new service bundle.
sdv_lifecycle_client::register_service_bundle!(ConCalServiceBundle);

impl ServiceBundle for ConCalServiceBundle {
    /// Creates a new instance of the ConCalServiceBundle.
    /// Called when service bundle is created by the system.
    ///
    /// Context object is provided as a parameter that gives access to the
    /// communication stack APIs.
    fn new(context: ContextRef) -> ConCalServiceBundle {
        ConCalServiceBundle { context, cancellation_token: None, background_thread: None }
    }

    /// Called when the service bundle is started by the system.
    fn on_start(&mut self) {
        info!("Starting ConCal service bundle...");
        let context = self.context;
        // Create a cancellation token and pass it down the thread to a runtime
        let cancellation_token = CancellationToken::new();
        self.cancellation_token = Some(cancellation_token.clone());
        self.background_thread = Some(thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
            rt.block_on(async {
                let concal = Arc::new(ConCalServiceBundleImpl::new());
                let _service = match ConCalServiceBundleMw::new(
                    Arc::new(SdvComms { context }),
                    (concal.clone(), concal.clone(), concal.clone(), concal.clone()),
                )
                .await
                {
                    Ok(service) => service,
                    Err(e) => panic!("{e}"),
                };
                Self::main_loop(cancellation_token.clone()).await;
            });
        }));
        info!("ConCal service bundle is running");
    }

    /// Called when the service bundle is stopped by the system in preparation
    /// for shutdown or suspend to RAM/Disc.
    fn on_stop(&mut self) {
        info!("Stopping ConCal service bundle");
        // Trigger cancellation token to exit Tokio runtime execution and drop it
        self.cancellation_token.as_mut().expect("Must have been created on_start").cancel();
        if let Some(thread) = self.background_thread.take() {
            let _ = thread.join();
        }
        self.cancellation_token = None;
        info!("ConCal service bundle stopped");
    }
}

/// Called when the service bundle is destroyed by the system.
impl Drop for ConCalServiceBundle {
    fn drop(&mut self) {
        self.on_stop();
    }
}
