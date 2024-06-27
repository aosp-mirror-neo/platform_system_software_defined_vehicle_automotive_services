// Copyright 2024 Google LLC

use crate::power_state_notifier::binder::BinderFeatures;
use crate::PowerState;
use anyhow::anyhow;
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
use log::{debug, error, info, warn};
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
use tokio::sync::watch::Receiver as WatchReceiver;
use tokio::task::JoinError;
use tokio::task::JoinHandle;
use tokio::task::JoinSet;

#[automock]
#[async_trait]
pub trait PowerStateNotifier: Send + Sync {
    fn create_binder_service(
        &self,
        power_state_watch_receiver: WatchReceiver<Arc<dyn PowerState>>,
    ) -> anyhow::Result<()>;
    async fn notify(&self, power_state_report: PowerStateReport) -> anyhow::Result<()>;
    fn notify_oem(
        &self,
        power_state_report: PowerStateReport,
        reason: PowerStateChangeReason,
    ) -> anyhow::Result<()>;
}

#[derive(Clone)]
pub struct PowerStateNotifierImpl {
    callbacks: Arc<Mutex<Vec<Strong<dyn IPowerStateReportListenerAsync<Tokio>>>>>,
    vpm_service_bundle: Arc<Mutex<VpmSystemServiceBundle>>,
}

impl PowerStateNotifierImpl {
    pub fn new(vpm_service_bundle: Arc<Mutex<VpmSystemServiceBundle>>) -> Self {
        Self { callbacks: Arc::new(Mutex::new(Vec::new())), vpm_service_bundle }
    }
}

#[async_trait]
impl PowerStateNotifier for PowerStateNotifierImpl {
    fn create_binder_service(
        &self,
        power_state_watch_receiver: WatchReceiver<Arc<dyn PowerState>>,
    ) -> anyhow::Result<()> {
        info!("VPM client binder starting!");
        let client_api_binder = BnPowerStateClientApi::new_async_binder(
            BinderServer::new(self.callbacks.clone(), power_state_watch_receiver),
            TokioRuntime(Handle::current()),
            BinderFeatures::default(),
        );
        binder::add_service(
            <BpPowerStateClientApi as IPowerStateClientApi>::get_descriptor(),
            client_api_binder.as_binder(),
        )?;
        Ok(())
    }

    async fn notify(&self, power_state_report: PowerStateReport) -> anyhow::Result<()> {
        info!("Starting callback notifications for {:?}", power_state_report);
        let cloned_callbacks = self.callbacks.lock().unwrap().clone();
        let cloned_bundle = self.vpm_service_bundle.clone();
        let mut set = JoinSet::new();
        for async_callback in cloned_callbacks.into_iter() {
            set.spawn(async move {
                async_callback.onPowerStateReportChanged(convert_to_aidl(power_state_report)).await
            });
        }
        let mut notification_failed = false;

        while let Some(res) = set.join_next().await {
            if (res.is_err()) {
                error!("Failed to notify callback {:?}!", res);
                debug!("Dropping the rest of the callback tasks!");
                notification_failed = true;
            } else if let Ok(Err(e)) = res {
                // This should be handled with death binder not here.
                notification_failed = true;
                error!("Callback handle was dropped {:?}", e);
            }
        }
        if notification_failed {
            Err(anyhow!("One or more callbacks failed"))
        } else {
            Ok(())
        }
    }

    fn notify_oem(
        &self,
        power_state_report: PowerStateReport,
        reason: PowerStateChangeReason,
    ) -> anyhow::Result<()> {
        let request = PowerStateNotificationRequest {
            power_state: power_state_report.into(),
            reason: reason.into(),
            ..Default::default()
        };

        // This should be changed to just getting a reference to the client rather than
        // sending the bundle here.

        let result = self
            .vpm_service_bundle
            .lock()
            .unwrap()
            .get_sdv_vpm_power_notification_service_client("VpmSystemClient")
            .notify_power_state(request);

        Ok(())
    }
}

struct BinderServer {
    callbacks: Arc<Mutex<Vec<Strong<dyn IPowerStateReportListenerAsync<Tokio>>>>>,
    power_state_watch_receiver: WatchReceiver<Arc<dyn PowerState>>,
}

impl BinderServer {
    fn new(
        callbacks: Arc<Mutex<Vec<Strong<dyn IPowerStateReportListenerAsync<Tokio>>>>>,
        power_state_watch_receiver: WatchReceiver<Arc<dyn PowerState>>,
    ) -> Self {
        Self { callbacks, power_state_watch_receiver }
    }
}
impl Interface for BinderServer {}

#[async_trait]
impl IPowerStateClientApiAsyncServer for BinderServer {
    async fn subscribeToPowerStateReport(
        &self,
        callback: &Strong<dyn IPowerStateReportListener>,
    ) -> BinderResult<PowerStateReportAidl> {
        info!("VPM Agent received client request to subscribe to power updates!");
        self.callbacks.lock().unwrap().push(callback.clone().into_async());
        let power_state = self.power_state_watch_receiver.borrow();
        Ok(convert_to_aidl(power_state.get_report()))
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
