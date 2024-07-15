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
use std::thread;
use std::time::Duration;

pub struct PowerCallbackPreparer {
    // The callback receiver will wait 10 seconds before returning success.
    // if power state report is set.
    sleep_callback_if_power_state: Option<PowerStateReportAidl>,
}

impl PowerCallbackPreparer {
    pub fn new() -> Self {
        Self { sleep_callback_if_power_state: None }
    }

    #[allow(dead_code)]
    pub fn with_sleep_on_state(
        &mut self,
        power_state_report: PowerStateReportAidl,
    ) -> &mut PowerCallbackPreparer {
        self.sleep_callback_if_power_state = Some(power_state_report);
        self
    }

    pub fn setup_binder_callbacks(&self) -> (PowerStateReportAidl, Receiver<PowerStateReportAidl>) {
        binder::ProcessState::start_thread_pool();
        let (callback_sender, callback_receiver): (
            Sender<PowerStateReportAidl>,
            Receiver<PowerStateReportAidl>,
        ) = channel();
        let test_sdv_agent = TestSdvAgent {
            callback_sender,
            sleep_callback_if_power_state: self.sleep_callback_if_power_state,
        };
        let power_state_client_interface =
            get_power_state_client_interface().expect("Cannot find VPM client binder service");
        let power_report_listener =
            BnPowerStateReportListener::new_binder(test_sdv_agent, BinderFeatures::default());
        // Subscribe to power state callback.
        let current_power_state_report = power_state_client_interface
            .subscribeToPowerStateReport(&power_report_listener)
            .unwrap();
        (current_power_state_report, callback_receiver)
    }
}

fn get_power_state_client_interface() -> anyhow::Result<Strong<dyn IPowerStateClientApi>> {
    Ok(binder::wait_for_interface::<dyn IPowerStateClientApi>(
        <BpPowerStateClientApi as IPowerStateClientApi>::get_descriptor(),
    )?)
}

pub struct TestSdvAgent {
    callback_sender: Sender<PowerStateReportAidl>,
    sleep_callback_if_power_state: Option<PowerStateReportAidl>,
}

impl binder::Interface for TestSdvAgent {}

impl IPowerStateReportListener for TestSdvAgent {
    fn onPowerStateReportChanged(&self, power_state: PowerStateReportAidl) -> binder::Result<()> {
        if self.sleep_callback_if_power_state.is_some()
            && self.sleep_callback_if_power_state.unwrap() == power_state
        {
            thread::sleep(Duration::from_secs(10));
        }
        self.callback_sender.send(power_state).unwrap();
        Ok(())
    }
}

pub struct TestVePSM {
    pub request_sender: Sender<PowerStateNotificationRequest>,
}

impl TestVePSM {
    pub fn create_vepsm_and_notification_receiver_channel(
    ) -> (Self, Receiver<PowerStateNotificationRequest>) {
        let (request_sender, request_receiver): (
            Sender<PowerStateNotificationRequest>,
            Receiver<PowerStateNotificationRequest>,
        ) = channel();

        (Self { request_sender }, request_receiver)
    }
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
