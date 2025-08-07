// Copyright 2024 Google LLC
//! Power state management client
use crate::binder::BinderFeatures;
use google_sdv_vpm_aidl::aidl::google::sdv::vpm::IPowerStateClientApi::IPowerStateClientApiAsync;
use google_sdv_vpm_aidl::aidl::google::sdv::vpm::IPowerStateClientApi::{
    BpPowerStateClientApi, IPowerStateClientApi,
};
use google_sdv_vpm_aidl::aidl::google::sdv::vpm::IPowerStateReportListener::{
    BnPowerStateReportListener, IPowerStateReportListenerAsyncServer,
};
use google_sdv_vpm_aidl::aidl::google::sdv::vpm::PowerStateReport::PowerStateReport;

use async_trait::async_trait;
use binder_tokio::Tokio;
use binder_tokio::TokioRuntime;
use google_sdv_vpm_aidl::binder;
use google_sdv_vpm_aidl::binder::Strong;
use std::future;
use tokio::runtime::Handle;
use tracing::instrument;

static LOG_TAG: &str = "vpm_client";

#[derive(Debug, Clone)]
struct VpmPowerStateClient {}

impl binder::Interface for VpmPowerStateClient {}

#[async_trait]
impl IPowerStateReportListenerAsyncServer for VpmPowerStateClient {
    async fn onPowerStateReportChanged(&self, power_state: PowerStateReport) -> binder::Result<()> {
        println!("VPM client received power state change {:?}", power_state);
        Ok(())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    sdv_log::init_logger(LOG_TAG)?;
    let power_state_client = VpmPowerStateClient {};

    binder::ProcessState::start_thread_pool();
    let vpm_client_service_interface =
        get_binder_service().await.expect("Cannot find VPM client binder service");

    let vpm_client_binder = BnPowerStateReportListener::new_async_binder(
        power_state_client,
        TokioRuntime(Handle::current()),
        BinderFeatures::default(),
    );
    let result = vpm_client_service_interface.subscribeToPowerStateReport(&vpm_client_binder).await;

    match result {
        Err(err) => panic!("Cannot subscribe to power state {err:?}"),
        Ok(current_power_state_report) => println!(
            "Successfully subscribed to vpm, current report {current_power_state_report:?}"
        ),
    }
    future::pending::<()>().await;
    Ok(())
}

#[instrument]
async fn get_binder_service() -> anyhow::Result<Strong<dyn IPowerStateClientApiAsync<Tokio>>> {
    let descriptor =
        <BpPowerStateClientApi as IPowerStateClientApi>::get_descriptor().to_owned() + "/default";
    Ok(binder_tokio::wait_for_interface::<dyn IPowerStateClientApiAsync<Tokio>>(&descriptor)
        .await?)
}
