// Copyright 2023 Google LLC
//! Vehicle Mode and Power Management Agent.

mod agent;
mod vehicle_state_listener;

use crate::agent::VPM;
use log::info;

static LOG_TAG: &str = "sdv_vpm_agent";

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> anyhow::Result<()> {
    sdv_log::init_logger(LOG_TAG)?;
    info!("VPM Agent starting!");
    let vpm = VPM::new();
    vpm.start().await;
    Ok(())
}
