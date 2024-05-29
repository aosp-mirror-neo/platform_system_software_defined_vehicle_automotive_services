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
use google_sdv_vpm_aidl::binder::{
    Interface, Result as BinderResult, Status as BinderStatus, Strong,
};
use log::{error, info, warn};
use mockall::automock;
use sdv_vpm_api::power::PowerStateReport;
use sdv_vpm_api::vpm::power_state_notification_request::PowerStateChangeReason;
use sdv_vpm_api::vpm::PowerStateNotificationRequest;
use sdvgenerated::sdv_vpm_power_notification_service_interface::PowerNotificationService;
use sdvgenerated::vpm_system_service_bundle::VpmSystemServiceBundle;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::runtime::Handle;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::task::JoinSet;

#[automock]
#[async_trait]
pub trait PowerStateNotifier: Send + Sync {
    fn create_binder_service(&self) -> anyhow::Result<()>;
    async fn start(&mut self) -> anyhow::Result<()>;
}

pub struct PowerStateNotifierImpl {
    callbacks: Arc<Mutex<Vec<Strong<dyn IPowerStateReportListenerAsync<Tokio>>>>>,
    receiver: Receiver<PowerStateReport>,
    vpm_service_bundle: Arc<Mutex<VpmSystemServiceBundle>>,
}

impl PowerStateNotifierImpl {
    pub fn new(
        receiver: Receiver<PowerStateReport>,
        vpm_service_bundle: Arc<Mutex<VpmSystemServiceBundle>>,
    ) -> Self {
        Self { callbacks: Arc::new(Mutex::new(Vec::new())), receiver, vpm_service_bundle }
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
            let cloned_bundle = self.vpm_service_bundle.clone();
            tokio::spawn(async move {
                let mut set = JoinSet::new();
                for async_callback in cloned_callbacks.into_iter() {
                    set.spawn(async move {
                        async_callback
                            .onPowerStateReportChanged(convert_to_aidl(power_state_report))
                            .await
                    });
                }
                let notification_failed = AtomicBool::new(false);
                while let Some(res) = set.join_next().await {
                    if (res.is_err()) {
                        error!("Failed to notify callback {:?}", res);
                        info!("Dropping the rest of the callbacks");
                        set.abort_all();
                        notification_failed.store(true, Ordering::Relaxed);
                    } else if let Ok(Err(e)) = res {
                        // This should be handled with death binder not here.
                        error!("Callback handle was dropped {:?}", e);
                    }
                }

                if (notification_failed.load(Ordering::Relaxed)) {
                    error!("one of the callbacks failed, not notifying OEM!");
                } else {
                    info!("Power state clients notified.");
                    let result = notify_oem(cloned_bundle, power_state_report);
                    if result.is_ok() {
                        info!("OEM notified {:?}", result);
                    } else {
                        error!("Failed to notify OEM, reason={:?}!", result.err());
                    }
                }
            });
        }
        Ok(())
    }
}

fn notify_oem(
    bundle: Arc<Mutex<VpmSystemServiceBundle>>,
    power_state_report: PowerStateReport,
) -> anyhow::Result<()> {
    let request = PowerStateNotificationRequest {
        power_state: power_state_report.into(),
        reason: PowerStateChangeReason::HOST_REQUESTED.into(),
        ..Default::default()
    };

    // This should be changed to just getting a reference to the client rather than
    // sending the bundle here.

    let result = bundle
        .lock()
        .unwrap()
        .get_sdv_vpm_power_notification_service_client("VpmSystemClient")
        .notify_power_state(request);

    Ok(())
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
