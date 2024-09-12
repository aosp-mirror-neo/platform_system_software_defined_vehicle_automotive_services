// Copyright 2024 Google LLC

//! Diagnostics agent implementation.

use log::info;

fn main() {
    sdv_log::init_logger("sdv_diagnostics_agent").unwrap();
    sdv_tracing::init_tracing();

    info!("Starting diagnostics");

    binder::ProcessState::start_thread_pool();

    binder::ProcessState::join_thread_pool();
}
