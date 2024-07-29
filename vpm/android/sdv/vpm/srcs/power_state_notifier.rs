// Copyright 2024 Google LLC

use crate::power_state_notifier::binder::BinderFeatures;
use crate::PowerState;
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
use log::{debug, error, info};
use mockall::automock;
use sdv_vpm_api::power::PowerStateReport;
use sdv_vpm_api::vpm::power_state_notification_request::PowerStateChangeReason;
use sdv_vpm_api::vpm::PowerStateNotificationRequest;
use sdvgenerated::sdv_vpm_power_notification_service_interface::PowerNotificationService;
use sdvgenerated::vpm_system_service_bundle::VpmSystemServiceBundle;
use std::sync::Arc;
use std::sync::Mutex;
use thiserror::Error;
use tokio::runtime::Handle;
use tokio::sync::watch::Receiver as WatchReceiver;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

#[automock]
#[async_trait]
pub trait PowerStateNotifier: Send + Sync {
    fn create_binder_service(
        &self,
        power_state_watch_receiver: WatchReceiver<Arc<dyn PowerState>>,
    ) -> anyhow::Result<()>;
    async fn notify(
        &self,
        power_state_report: PowerStateReport,
        cancellation_token: CancellationToken,
    ) -> anyhow::Result<(), PowerStateNotificationError>;
    fn notify_oem(
        &self,
        power_state_report: PowerStateReport,
        reason: PowerStateChangeReason,
    ) -> anyhow::Result<()>;
}

#[derive(Error, Debug, Clone)]
pub enum PowerStateNotificationError {
    #[error("Notification was cancelled by caller!")]
    Cancelled,
    #[error("Callback client was disconnected!")]
    Disconnected,
    #[error("Failed to deliver callback!")]
    Failed,
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

    async fn notify(
        &self,
        power_state_report: PowerStateReport,
        cancellation_token: CancellationToken,
    ) -> anyhow::Result<(), PowerStateNotificationError> {
        info!("Starting callback notifications for {:?}", power_state_report);
        let cloned_callbacks = self.callbacks.lock().unwrap().clone();
        let mut set = JoinSet::new();
        for async_callback in cloned_callbacks.into_iter() {
            let cloned_cancellation_token = cancellation_token.clone();
            if cancellation_token.is_cancelled() {
                debug!("Cancelling callbacks for {:?}", power_state_report);
                return Err(PowerStateNotificationError::Cancelled);
            }

            set.spawn(async move {
                if cloned_cancellation_token.is_cancelled() {
                    debug!("Cancelling callbacks for {:?}", power_state_report);
                    Err(PowerStateNotificationError::Cancelled)
                } else {
                    let result = async_callback
                        .onPowerStateReportChanged(convert_to_aidl(power_state_report))
                        .await;
                    if result.is_err() {
                        Err(PowerStateNotificationError::Disconnected)
                    } else {
                        Ok(())
                    }
                }
            });
        }
        let mut notification_failed = false;
        while let (false, Some(handle)) = (cancellation_token.is_cancelled(), set.join_next().await)
        {
            if handle.is_err() {
                error!("Power notification callback task failed, {:?}", handle.err());
                notification_failed = true;
                continue;
            }
            if let Err(result) = handle {
                error!("Power notification callback request failed, {:?}", result);
                notification_failed = true;
            }
        }

        if cancellation_token.is_cancelled() {
            debug!(
                "Power state notification callbacks were cancelled for {:?}",
                power_state_report
            );
            return Err(PowerStateNotificationError::Cancelled);
        }

        if notification_failed {
            return Err(PowerStateNotificationError::Failed);
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

        // Currently it either succeeds or panics.
        debug!("OEM notification server was found {:?}", result);

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
