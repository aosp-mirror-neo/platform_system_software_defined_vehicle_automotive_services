// Copyright 2023 Google LLC
//! Diagnostics Manager empty sample implementation.

mod dm;
mod dm_async;
mod not_implemented;

use android_sdv_diagnostics::aidl::android::sdv::diagnostics::IDiagnosticsManager::BnDiagnosticsManager;
use android_sdv_diagnostics::binder::{self, BinderFeatures};
use binder_tokio::TokioRuntime;
use clap::Parser;
use log::info;
use rpcbinder::RpcServer;
use tokio::runtime::Handle;

use crate::dm::DiagnosticsManager;
use crate::dm_async::DiagnosticsManagerAsync;

static LOG_TAG: &str = "diag_manager";

#[derive(Parser)]
struct CliArgs {
    #[arg(long = "async")]
    use_async_server: bool,
}

fn get_dm_async() -> binder::SpIBinder {
    BnDiagnosticsManager::new_async_binder(
        DiagnosticsManagerAsync,
        TokioRuntime(Handle::current()),
        BinderFeatures::default(),
    )
    .as_binder()
}

fn get_dm() -> binder::SpIBinder {
    BnDiagnosticsManager::new_binder(DiagnosticsManager, BinderFeatures::default()).as_binder()
}

fn register_dm_binder(dm_binder: binder::SpIBinder) -> anyhow::Result<()> {
    info!("Registering diagnostics manager");

    let server = RpcServer::new_vsock(dm_binder, libc::VMADDR_CID_ANY, /* port = */ 5678u32)?;

    info!("Calling join on server");

    server.join();
    Ok(())
}

fn start_dm_async() -> anyhow::Result<()> {
    info!("Async diagnostics implementation registration");

    register_dm_binder(get_dm_async())
}

fn start_dm() -> anyhow::Result<()> {
    info!("Sync diagnostics implementation registration");
    register_dm_binder(get_dm())
}

/// Run sample on Tokio runtime with 2 thread to enable basic concurrency.
#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> anyhow::Result<()> {
    sdv_log::init_logger(LOG_TAG)?;

    let use_async_server = CliArgs::parse().use_async_server;

    info!("Diagnostics Manager starting! [async = {}]", use_async_server);
    if use_async_server {
        start_dm_async()?;
    } else {
        start_dm()?;
    }

    Ok(())
}
