// Copyright 2024 Google LLC
//! Vehicle Mode and Power Management Agent.

use google_sdv_vpm_aidl::binder;
use log::error;
use sdv_vpm::Agent;
use tokio::task;

static LOG_TAG: &str = "sdv_vpm_agent";

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> anyhow::Result<()> {
    sdv_log::init_logger(LOG_TAG)?;
    sdv_tracing::init_tracing();
    binder::ProcessState::start_thread_pool();

    let res = sdv_comms_ctx::wait_for_agents();
    if res.is_err() {
        error!("wait_for_agents() failed with error: {:?}", res.unwrap_err());
    }

    sdv_vpm::create_and_start_agent(Agent::new).await?;

    task::block_in_place(move || {
        binder::ProcessState::join_thread_pool();
    });
    Ok(())
}
