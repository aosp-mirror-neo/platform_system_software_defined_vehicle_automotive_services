use crate::power_state_notifier::binder::BinderFeatures;
use async_trait::async_trait;
use binder_tokio::Tokio;
use binder_tokio::TokioRuntime;
use google_sdv_vpm_aidl::aidl::google::sdv::vpm::IPowerStateClientApi::IPowerStateClientApiAsyncServer;
use google_sdv_vpm_aidl::aidl::google::sdv::vpm::IPowerStateClientApi::{
    BnPowerStateClientApi, BpPowerStateClientApi, IPowerStateClientApi,
};
use google_sdv_vpm_aidl::aidl::google::sdv::vpm::IPowerStateReportListener::IPowerStateReportListener;
use google_sdv_vpm_aidl::aidl::google::sdv::vpm::IPowerStateReportListener::IPowerStateReportListenerAsync;
use google_sdv_vpm_aidl::aidl::google::sdv::vpm::PowerStateReport::PowerStateReport as PowerStateReportAidl;
use google_sdv_vpm_aidl::binder;
use google_sdv_vpm_aidl::binder::{Interface, Result as BinderResult, Strong};
use log::{error, info, warn};
use mockall::automock;
use sdv_vpm_api::power::PowerStateReport;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::runtime::Handle;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc::Receiver;

#[automock]
#[async_trait]
pub trait PowerStateNotifier: Send + Sync {
    fn create_binder_service(&self) -> anyhow::Result<()>;
    async fn start(&mut self) -> anyhow::Result<()>;
}

pub struct PowerStateNotifierImpl {
    callbacks: Arc<Mutex<Vec<Strong<dyn IPowerStateReportListenerAsync<Tokio>>>>>,
    receiver: Receiver<PowerStateReport>,
}

impl PowerStateNotifierImpl {
    pub fn new(receiver: Receiver<PowerStateReport>) -> Self {
        Self { callbacks: Arc::new(Mutex::new(Vec::new())), receiver }
    }
}

#[async_trait]
impl PowerStateNotifier for PowerStateNotifierImpl {
    fn create_binder_service(&self) -> anyhow::Result<()> {
        info!("VPM client binder starting!");
        let client_api_binder = BnPowerStateClientApi::new_async_binder(
            BinderServer::new(self.callbacks.clone()),
            TokioRuntime(Handle::current()),
            BinderFeatures::default(),
        );
        binder::add_service(
            <BpPowerStateClientApi as IPowerStateClientApi>::get_descriptor(),
            client_api_binder.as_binder(),
        )?;
        Ok(())
    }

    async fn start(&mut self) -> anyhow::Result<()> {
        while let Some(power_state_report) = self.receiver.recv().await {
            info!("Starting callback notifications for {:?}", power_state_report);
            let cloned_callbacks = self.callbacks.lock().unwrap().clone();
            for async_callback in cloned_callbacks.into_iter() {
                tokio::spawn(async move {
                    async_callback
                        .onPowerStateReportChanged(convert_to_aidl(power_state_report))
                        .await;
                });
            }
        }
        Ok(())
    }
}

struct BinderServer {
    callbacks: Arc<Mutex<Vec<Strong<dyn IPowerStateReportListenerAsync<Tokio>>>>>,
}

impl BinderServer {
    fn new(callbacks: Arc<Mutex<Vec<Strong<dyn IPowerStateReportListenerAsync<Tokio>>>>>) -> Self {
        Self { callbacks }
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
        self.callbacks.lock().unwrap().push(callback.clone().into_async());
        Ok(())
    }
}

fn convert_to_aidl(power_state_report: PowerStateReport) -> PowerStateReportAidl {
    match power_state_report {
        PowerStateReport::POWER_OFF_EXIT => PowerStateReportAidl::POWER_OFF_EXIT,
        PowerStateReport::SUSPEND_TO_RAM_EXIT => PowerStateReportAidl::SUSPEND_TO_RAM_EXIT,
        PowerStateReport::SUSPEND_TO_DISK_EXIT => PowerStateReportAidl::SUSPEND_TO_DISK_EXIT,
        PowerStateReport::ON => PowerStateReportAidl::ON,
        PowerStateReport::SHUTDOWN_CANCELLED => PowerStateReportAidl::SHUTDOWN_CANCELLED,
        PowerStateReport::POWER_OFF_ENTER => PowerStateReportAidl::POWER_OFF_ENTER,
        PowerStateReport::SUSPEND_TO_RAM_ENTER => PowerStateReportAidl::SUSPEND_TO_RAM_ENTER,
        PowerStateReport::SUSPEND_TO_DISK_ENTER => PowerStateReportAidl::SUSPEND_TO_DISK_ENTER,
        PowerStateReport::POWER_OFF_POST_FINISH => PowerStateReportAidl::POWER_OFF_POST_FINISH,
        PowerStateReport::SUSPEND_TO_RAM_POST_FINISH => {
            PowerStateReportAidl::SUSPEND_TO_RAM_POST_FINISH
        }
        PowerStateReport::SUSPEND_TO_DISK_POST_FINISH => {
            PowerStateReportAidl::SUSPEND_TO_DISK_POST_FINISH
        }
        PowerStateReport::WAIT_FOR_FINISH => PowerStateReportAidl::WAIT_FOR_FINISH,
        PowerStateReport::REPORT_UNSPECIFIED => panic!("This conversion should never be called!"),
    }
}
