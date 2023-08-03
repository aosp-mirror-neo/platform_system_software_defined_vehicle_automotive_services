// Copyright 2023 Google LLC

use crate::vpm_system_service::VPMSystemService;
use android_sdv_vpm_aidl::aidl::android::sdv::vpm::ICustomStateListener::{
    ICustomStateListener, ICustomStateListenerAsync,
};
use android_sdv_vpm_aidl::aidl::android::sdv::vpm::IPowerStateListener::{
    IPowerStateListener, IPowerStateListenerAsync,
};
use android_sdv_vpm_aidl::aidl::android::sdv::vpm::IVpmClientApi::IVpmClientApiAsyncServer;
use android_sdv_vpm_aidl::aidl::android::sdv::vpm::PowerState::PowerState;
use android_sdv_vpm_aidl::binder;
use android_sdv_vpm_aidl::binder::{Interface, Result as BinderResult, Strong};
use async_trait::async_trait;
use binder_tokio::Tokio;
use log::{error, info, warn};
use tokio::sync::broadcast::error::RecvError;

#[derive(Clone)]
pub struct VpmClientServiceApi<'a> {
    system_service: &'a VPMSystemService,
}

impl<'a> VpmClientServiceApi<'a> {
    pub fn new(vpm_system_service: &'a VPMSystemService) -> VpmClientServiceApi<'a> {
        VpmClientServiceApi { system_service: vpm_system_service }
    }
}

impl Interface for VpmClientServiceApi<'_> {}

#[async_trait]
impl IVpmClientApiAsyncServer for VpmClientServiceApi<'_> {
    async fn getPowerState(&self) -> BinderResult<PowerState> {
        Ok(self.system_service.get_power_state())
    }

    async fn subscribeToPowerStateChange(
        &self,
        power_state_listener: &Strong<dyn IPowerStateListener>,
    ) -> BinderResult<()> {
        info!("Agent received client request to subscribe to power updates!");
        let mut receiver = self.system_service.subscribe_to_power_updates();
        let cloned_listener: Strong<dyn IPowerStateListenerAsync<Tokio>> =
            power_state_listener.clone().into_async();
        tokio::spawn(async move {
            loop {
                //TODO(b/293833738): Add death binder and unsubscribe features.
                match receiver.recv().await {
                    Ok(power_state) => {
                        info!("Notifying subscriber for power state = {:?}", power_state);
                        let result =
                            cloned_listener.onPowerStateChanged(power_state).await.unwrap();
                        info!("Notification sent to power subscriber {:?}", result);
                    }
                    Err(RecvError::Lagged(missed)) => {
                        warn!("A receiver is lagging behind with {:?} messages", missed);
                    }
                    Err(RecvError::Closed) => {
                        error!("Sender was dropped, stopping power state listener task!");
                        break;
                    }
                }
            }
        });
        Ok(())
    }

    async fn subscribeToCustomStateChange(
        &self,
        state_machine_name: &str,
        callback: &Strong<dyn ICustomStateListener>,
    ) -> BinderResult<()> {
        let copied_state_machine_name = state_machine_name.to_string();
        info!(
            "Agent received client request to subscribe custom state update {:?}",
            &copied_state_machine_name
        );
        let mut receiver =
            match self.system_service.subscribe_to_custom_updates(&copied_state_machine_name) {
                Some(receiver) => receiver,
                None => {
                    error!(
                "returning error because receiver could not be created for custom state {:?}",
                copied_state_machine_name
            );
                    return Err(binder::StatusCode::BAD_VALUE.into());
                }
            };
        let cloned_listener: Strong<dyn ICustomStateListenerAsync<Tokio>> =
            callback.clone().into_async();
        tokio::spawn(async move {
            loop {
                //TODO(b/293833738): Add death binder and unsubscribe features.
                match receiver.recv().await {
                    Ok(custom_state_value) => {
                        info!(
                            "Notifying subscriber for custom state = {:?}",
                            copied_state_machine_name
                        );
                        let result = cloned_listener
                            .onCustomStateChanged(&copied_state_machine_name, &custom_state_value)
                            .await
                            .unwrap();
                        info!("Notification sent to custom state subscriber {:?}", result);
                    }
                    Err(RecvError::Lagged(missed)) => {
                        warn!("A receiver is lagging behind with {:?} messages", missed);
                    }
                    Err(RecvError::Closed) => {
                        error!("Sender was dropped, stopping custom state listener task!");
                        break;
                    }
                }
            }
        });
        Ok(())
    }
}
