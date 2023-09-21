// Copyright 2023 Google LLC
//! Diagnostics service empty sample implementation

use android_sdv_diagnostics::aidl::android::sdv::diagnostics::{
    event::EventStatus::EventStatus, IDiagnosticsManager::IDiagnosticsManager,
};
use android_sdv_diagnostics::binder::Strong;
use clap::Parser;
use log::info;
use rpcbinder::RpcSession;
use std::error::Error;

static LOG_TAG: &str = "diag_service";

#[derive(Parser)]
struct ServiceArgs {
    /// The cid of the service host
    #[arg(short = 's', long)]
    dm_host_cid: u32,

    /// The port of the service
    #[arg(short = 'p', long)]
    dm_port: u32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    sdv_log::init_logger(LOG_TAG)?;
    info!("Diagnostics service starting!");

    let service_args = ServiceArgs::parse();

    let diag_manager = get_diag_manager(service_args.dm_host_cid, service_args.dm_port)?;
    info!("After getting diag manager");
    let result = diag_manager.setEventStatus("event_id", EventStatus::PRE_FAILED);

    info!("Getting result after setting event status: {:#?}", result);

    Ok(())
}

fn get_diag_manager(cid: u32, port: u32) -> anyhow::Result<Strong<dyn IDiagnosticsManager>> {
    info!("Getting diag manager with {cid}: port {port}");
    Ok(RpcSession::new().setup_vsock_client(cid, port)?)
}
