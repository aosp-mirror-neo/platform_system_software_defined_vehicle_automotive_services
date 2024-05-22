use crate::power_state_notifier::binder::BinderFeatures;
use async_trait::async_trait;
use binder_tokio::Tokio;
use binder_tokio::TokioRuntime;
use google_sdv_vpm_aidl::aidl::google::sdv::vpm::IPowerStateClientApi::IPowerStateClientApiAsyncServer;
use google_sdv_vpm_aidl::aidl::google::sdv::vpm::IPowerStateClientApi::{
    BnPowerStateClientApi, BpPowerStateClientApi, IPowerStateClientApi,
};
use google_sdv_vpm_aidl::aidl::google::sdv::vpm::IPowerStateReportListener::IPowerStateReportListener;
use google_sdv_vpm_aidl::aidl::google::sdv::vpm::PowerStateReport::PowerStateReport;
use google_sdv_vpm_aidl::binder;
use google_sdv_vpm_aidl::binder::{Interface, Result as BinderResult, Strong};
use log::{error, info, warn};
use mockall::automock;
use tokio::runtime::Handle;
use tokio::sync::broadcast::error::RecvError;

#[automock]
#[async_trait]
pub trait PowerStateNotifier: Send + Sync {
    fn create_binder_service(&self) -> anyhow::Result<()>;
}

pub struct PowerStateNotifierImpl {}

impl PowerStateNotifierImpl {
    pub fn new() -> Self {
        Self {}
    }
}

impl PowerStateNotifier for PowerStateNotifierImpl {
    fn create_binder_service(&self) -> anyhow::Result<()> {
        info!("VPM client binder starting!");
        let client_api_binder = BnPowerStateClientApi::new_async_binder(
            BinderServer::new(),
            TokioRuntime(Handle::current()),
            BinderFeatures::default(),
        );
        binder::add_service(
            <BpPowerStateClientApi as IPowerStateClientApi>::get_descriptor(),
            client_api_binder.as_binder(),
        )?;
        Ok(())
    }
}

struct BinderServer {}

impl BinderServer {
    fn new() -> Self {
        Self {}
    }
}
impl Interface for BinderServer {}

#[async_trait]
impl IPowerStateClientApiAsyncServer for BinderServer {
    async fn subscribeToPowerStateReport(
        &self,
        callback: &Strong<dyn IPowerStateReportListener>,
    ) -> BinderResult<()> {
        info!("VPM Agent received client request to subscribe to power updates!");
        //TODO: implement subscription handler
        Ok(())
    }
}
