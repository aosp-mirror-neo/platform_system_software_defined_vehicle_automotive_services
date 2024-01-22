// Copyright 2023 Google LLC
//! Vehicle and Power Mode Service.

use log::info;
static LOG_TAG: &str = "sdv_vpm_agent";
use std::future;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    sdv_log::init_logger(LOG_TAG)?;
    info!("VPM Agent starting!");

    //TODO: implement VPM Agent
    future::pending::<()>().await;
    Ok(())
}
