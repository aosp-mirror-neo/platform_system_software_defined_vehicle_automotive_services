// Copyright 2023 Google LLC
//! Vehicle and Power Mode Service.

mod vpm_client_service_api;
mod vpm_system_service;
mod vpm_system_service_api;
use crate::binder::BinderFeatures;
use crate::vpm_client_service_api::VpmClientServiceApi;
use crate::vpm_system_service::VPMSystemService;
use android_sdv_vpm_aidl::aidl::android::sdv::vpm::IVpmClientApi::{
    BnVpmClientApi, BpVpmClientApi, IVpmClientApi,
};
use android_sdv_vpm_aidl::aidl::android::sdv::vpm::IVpmSystemApi::{
    BpVpmSystemApi, IVpmSystemApi, SERVICE_NAME, SERVICE_PORT,
};
use android_sdv_vpm_aidl::binder;
use binder_tokio::TokioRuntime;
use log::info;
use rpcbinder::RpcServer;
use sdv_sd::{ServiceDefinitionBuilder, ServiceManager};
use std::sync::OnceLock;
use tokio::runtime::Handle;
use tokio::task;
use tracing::instrument;

static LOG_TAG: &str = "sdv_vpm_agent";
static CELL: OnceLock<VPMSystemService> = OnceLock::new();

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> anyhow::Result<()> {
    sdv_log::init_logger(LOG_TAG)?;
    sdv_tracing::init_tracing();

    info!("VPM Agent starting!");

    let system_service: &VPMSystemService = CELL.get_or_init(VPMSystemService::new);

    binder::ProcessState::start_thread_pool();

    create_system_api_binder_in_service_discovery(system_service)?;

    create_system_api_binder_service(system_service)?;

    create_client_api_binder_service(system_service)?;

    task::block_in_place(move || {
        binder::ProcessState::join_thread_pool();
    });

    Ok(())
}

// This is another example where we use binder::addService which currently does not use SD.
#[instrument(skip(system_service_reference))]
fn create_system_api_binder_service(
    system_service_reference: &'static VPMSystemService,
) -> anyhow::Result<()> {
    info!("VPM system service starting on binder!");
    binder::add_service(
        <BpVpmSystemApi as IVpmSystemApi>::get_descriptor(),
        vpm_system_service_api::create_system_binder(system_service_reference)?,
    )?;
    Ok(())
}

// This is an example if we need to use SD and then use binder-rpc.
// It uses binder-rpc/vsock
#[instrument(skip(system_service_reference))]
fn create_system_api_binder_in_service_discovery(
    system_service_reference: &'static VPMSystemService,
) -> anyhow::Result<()> {
    info!("VPM system service starting on SD!");
    RpcServer::new_vsock(
        vpm_system_service_api::create_system_binder(system_service_reference)?,
        libc::VMADDR_CID_LOCAL,
        u32::try_from(SERVICE_PORT)?,
    )?
    .start();

    let service_definition = ServiceDefinitionBuilder::new(SERVICE_NAME)
        .add_endpoint("binder_rpc/vsock".into(), "1".to_owned(), 0)
        .build();
    let sm = ServiceManager::default_service_manager();
    sm.add_service(&service_definition)?;
    info!("VPM registered to service discovery.");
    Ok(())
}

// Create the client api service that allows clients to subscribe to power state machines and custom state machine updates.
#[instrument(skip(system_service))]
fn create_client_api_binder_service(
    system_service: &'static VPMSystemService,
) -> anyhow::Result<()> {
    info!("VPM client api service starting on binder!");
    let service_api_binder = BnVpmClientApi::new_async_binder(
        VpmClientServiceApi::new(system_service),
        TokioRuntime(Handle::current()),
        BinderFeatures::default(),
    );

    binder::add_service(
        <BpVpmClientApi as IVpmClientApi>::get_descriptor(),
        service_api_binder.as_binder(),
    )?;
    Ok(())
}
