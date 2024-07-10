// Copyright 2024 Google LLC
use crate::common::binder::BinderFeatures;
use crate::common::binder::Strong;
use sdvgenerated::sdv_vpm_power_notification_service_interface::PowerNotificationService;

use google_sdv_vpm_aidl::aidl::google::sdv::vpm::IPowerStateClientApi::{
    BpPowerStateClientApi, IPowerStateClientApi,
};
use google_sdv_vpm_aidl::aidl::google::sdv::vpm::IPowerStateReportListener::{
    BnPowerStateReportListener, IPowerStateReportListener,
};
use google_sdv_vpm_aidl::aidl::google::sdv::vpm::PowerStateReport::PowerStateReport as PowerStateReportAidl;
use google_sdv_vpm_aidl::binder;
use sdv_vpm_api::vpm::PowerStateNotificationRequest;
use sdv_vpm_api::vpm::PowerStateNotificationResponse;
use sdvmiddleware_rpc::RpcResult;
use std::sync::mpsc::{channel, Receiver, Sender};

pub fn setup_binder_callbacks_for_power_state_listeners(
) -> (PowerStateReportAidl, Receiver<PowerStateReportAidl>) {
    binder::ProcessState::start_thread_pool();
    let (callback_sender, callback_receiver): (
        Sender<PowerStateReportAidl>,
        Receiver<PowerStateReportAidl>,
    ) = channel();
    let test_sdv_agent = TestSdvAgent { callback_sender };
    let power_state_client_interface =
        get_power_state_client_interface().expect("Cannot find VPM client binder service");
    let power_report_listener =
        BnPowerStateReportListener::new_binder(test_sdv_agent, BinderFeatures::default());
    // Subscribe to power state callback.
    let current_power_state_report =
        power_state_client_interface.subscribeToPowerStateReport(&power_report_listener).unwrap();
    (current_power_state_report, callback_receiver)
}

fn get_power_state_client_interface() -> anyhow::Result<Strong<dyn IPowerStateClientApi>> {
    Ok(binder::wait_for_interface::<dyn IPowerStateClientApi>(
        <BpPowerStateClientApi as IPowerStateClientApi>::get_descriptor(),
    )?)
}
pub struct TestSdvAgent {
    callback_sender: Sender<PowerStateReportAidl>,
}

impl binder::Interface for TestSdvAgent {}

impl IPowerStateReportListener for TestSdvAgent {
    fn onPowerStateReportChanged(&self, power_state: PowerStateReportAidl) -> binder::Result<()> {
        self.callback_sender.send(power_state).unwrap();
        Ok(())
    }
}

pub struct TestVePSM {
    pub request_sender: Sender<PowerStateNotificationRequest>,
}

impl PowerNotificationService for TestVePSM {
    fn notify_power_state(
        &self,
        request: PowerStateNotificationRequest,
    ) -> RpcResult<PowerStateNotificationResponse> {
        self.request_sender.send(request).unwrap();
        Ok(PowerStateNotificationResponse { ..Default::default() })
    }
}
