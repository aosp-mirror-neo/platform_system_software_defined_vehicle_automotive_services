// Copyright 2024 Google LLC

use crate::power_state_notifier::binder::BinderFeatures;
use crate::power_state_notifier::binder::ExceptionCode;
use crate::power_state_notifier::binder::Status;
use crate::power_state_notifier::binder::ThreadState;
use crate::PowerState;
use anyhow::anyhow;
use async_trait::async_trait;
use binder::{DeathRecipient, IBinder};
use binder_tokio::Tokio;
use binder_tokio::TokioRuntime;
use com_sdv_google_vpm_client_power_notification_service_rpc::Client;
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
use sdv_mw_rs_comms::{ClientAttributes, ClientOptions, Communicate, RpcClient, UnitName};
use sdv_mw_rs_sdv_comms::SdvComms;
use sdv_status_rs::SdvStatusCode;
use sdv_vpm_api::power::PowerStateReport;
use sdv_vpm_api::vpm_notifications::power_state_notification_request::PowerStateChangeReason;
use sdv_vpm_api::vpm_notifications::PowerStateNotificationRequest;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use thiserror::Error;
use tokio::runtime::Handle;
use tokio::sync::watch::Receiver as WatchReceiver;
use tokio::sync::Mutex as AsyncMutex;
use tokio::task::JoinSet;
use tracing::instrument;

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
    ) -> anyhow::Result<(), PowerStateNotificationError>;
    async fn notify_oem(
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
    #[error("Timeout was reached while notifying callbacks!")]
    TimedOut,
    #[error("Failed to deliver callback!")]
    Failed,
}

#[derive(Clone)]
pub struct PowerStateNotifierImpl {
    // Maps the AID of the agent with the callback.
    callbacks: Arc<Mutex<HashMap<u32, ClientCallback>>>,
    comms: Arc<SdvComms>,
    notification_rpc_client: Arc<AsyncMutex<Option<Arc<Client>>>>,
}

impl PowerStateNotifierImpl {
    pub fn new(comms: Arc<SdvComms>) -> Self {
        Self {
            callbacks: Arc::new(Mutex::new(HashMap::new())),
            comms,
            notification_rpc_client: Arc::new(AsyncMutex::new(None)),
        }
    }

    pub async fn get_notification_client(&self) -> anyhow::Result<Arc<Client>> {
        let client_guard = self.notification_rpc_client.lock().await;
        let client = client_guard.clone();
        client.ok_or_else(|| anyhow!("No notification client is stored"))
    }

    async fn update_client(&self) -> anyhow::Result<()> {
        let mut client = self.notification_rpc_client.lock().await;

        let server_name = UnitName::builder()
            .vm_name(self.comms.context.get_self_fqin().get_sdv_vm_name())
            .package_name("com.sdv.google.vpm.client")
            .bundle_name("VpmSystemClient")
            .service_unit_name(Client::DEFAULT_UNIT_NAME)
            .build()
            .map_err(|_| anyhow!("invalid server UnitName"))?;

        let attributes = ClientAttributes::builder()
            .server_name(server_name)
            .client_options(ClientOptions::default())
            .build()
            .map_err(|_| anyhow!("invalid ClientAttributes"))?;

        let result = self.comms.create_client_transport(&attributes).await;
        if result.is_err() {
            error!("Could not get a client for vpm notifications");
            return Err(anyhow!("Could not get a client for vpm notifications"));
        }
        *client = Some(Client::new(result.unwrap()).into());
        Ok(())
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

    #[instrument(skip(self))]
    async fn notify(
        &self,
        power_state_report: PowerStateReport,
    ) -> anyhow::Result<(), PowerStateNotificationError> {
        info!("Starting callback notifications for {:?}", power_state_report);

        let callbacks: Vec<_> = self
            .callbacks
            .lock()
            .unwrap()
            .iter()
            .map(|(id, client_callback)| (*id, client_callback.async_callback.clone()))
            .collect();

        let mut set = JoinSet::new();
        for (id, async_callback) in callbacks.into_iter() {
            set.spawn(async move {
                debug!("Starting callback notification for {:?}", id);
                let result = async_callback
                    .onPowerStateReportChanged(convert_to_aidl(power_state_report))
                    .await;
                if result.is_err() {
                    Err(PowerStateNotificationError::Disconnected)
                } else {
                    Ok(())
                }
            });
        }
        let mut notification_failed = false;
        while let Some(handle) = set.join_next().await {
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

        if notification_failed {
            return Err(PowerStateNotificationError::Failed);
        } else {
            Ok(())
        }
    }

    #[instrument(skip(self))]
    async fn notify_oem(
        &self,
        power_state_report: PowerStateReport,
        reason: PowerStateChangeReason,
    ) -> anyhow::Result<()> {
        let request = PowerStateNotificationRequest {
            power_state: power_state_report.into(),
            reason: reason.into(),
            ..Default::default()
        };

        if self.get_notification_client().await.is_err() {
            debug!("Updating the notification client");
            self.update_client().await?;
            debug!("Successfully created a new OEM notification client");
        }

        // Call the notification server.
        let client = self.get_notification_client().await?;
        let mut result = client.NotifyPowerState(&request).await;

        // Only retry if the client was unavailable.
        if let Err(ref sdv_status) = result {
            if sdv_status.status_code() == SdvStatusCode::Unavailable {
                debug!("Updating the notification client because it was unavailable");
                self.update_client().await?;
                result = self.get_notification_client().await?.NotifyPowerState(&request).await;
            }
        }
        if result.is_err() {
            return Err(anyhow!("Request to notify VPM client failed {:?}", result));
        }
        Ok(())
    }
}

struct BinderServer {
    callbacks: Arc<Mutex<HashMap<u32, ClientCallback>>>,
    power_state_watch_receiver: WatchReceiver<Arc<dyn PowerState>>,
}

impl BinderServer {
    fn new(
        callbacks: Arc<Mutex<HashMap<u32, ClientCallback>>>,
        power_state_watch_receiver: WatchReceiver<Arc<dyn PowerState>>,
    ) -> Self {
        Self { callbacks, power_state_watch_receiver }
    }
}
impl Interface for BinderServer {}

#[allow(dead_code)]
struct ClientCallback {
    async_callback: Strong<dyn IPowerStateReportListenerAsync<Tokio>>,
    // death_recipient has to be kept alive as long as the callback event if not used.
    death_recipient: DeathRecipient,
}

#[async_trait]
impl IPowerStateClientApiAsyncServer for BinderServer {
    async fn subscribeToPowerStateReport(
        &self,
        callback: &Strong<dyn IPowerStateReportListener>,
    ) -> BinderResult<PowerStateReportAidl> {
        info!("VPM Agent received client request to subscribe to power updates!");
        let id = ThreadState::get_calling_uid();
        let callbacks = self.callbacks.clone();
        let mut death_recipient = DeathRecipient::new(move || {
            info!("Handling death notification for id {:?}", id);
            callbacks.lock().unwrap().remove(&id);
        });
        let result = callback.as_binder().clone().link_to_death(&mut death_recipient);
        if result.is_err() {
            error!("Failed to link death binder {:?}", result);
            return Err(Status::new_exception(ExceptionCode::SERVICE_SPECIFIC, None));
        }

        debug!("Death binder linked successfully");
        let client_callback =
            ClientCallback { async_callback: callback.clone().into_async(), death_recipient };

        self.callbacks.lock().unwrap().insert(id, client_callback);
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
