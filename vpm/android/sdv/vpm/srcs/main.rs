// Copyright 2024 Google LLC
//! Vehicle Mode and Power Management Agent.

use log::info;
use sdv_vpm::VPM;

static LOG_TAG: &str = "sdv_vpm_agent";

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> anyhow::Result<()> {
    sdv_log::init_logger(LOG_TAG)?;
    info!("VPM Agent starting!");
    let vpm = VPM::new();
    vpm.start().await;
    Ok(())
}
