// Copyright 2024 Google LLC
//! Vehicle Mode and Power Management Agent.

use google_sdv_vpm_aidl::binder;
use sdv_vpm::Agent;
use tokio::task;

static LOG_TAG: &str = "sdv_vpm_agent";

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> anyhow::Result<()> {
    sdv_log::init_logger(LOG_TAG)?;
    binder::ProcessState::start_thread_pool();

    tokio::spawn(async {
        Agent::new().start().await;
    });

    task::block_in_place(move || {
        binder::ProcessState::join_thread_pool();
    });
    Ok(())
}
