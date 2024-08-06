// Copyright 2024 Google LLC
#![allow(dead_code)]

use crate::common::binder::BinderFeatures;
use crate::common::binder::Strong;
use google_sdv_vpm_aidl::aidl::google::sdv::vpm::IPowerStateClientApi::{
    BpPowerStateClientApi, IPowerStateClientApi,
};
use google_sdv_vpm_aidl::aidl::google::sdv::vpm::IPowerStateReportListener::{
    BnPowerStateReportListener, IPowerStateReportListener,
};
use sdv_middleware_pubsub::Result;
use sdv_vpm_api::power::PowerStateReport;
use sdv_vpm_api::vehicle::VehicleStateChange;
use sdv_vpm_api::vehicle::VpmVehicleState;
use sdv_vpm_api::vpm::power_state_notification_request::PowerStateChangeReason;
use sdv_vpm_api::vpm::PowerStateChangeRequest;
use sdv_vpm_api::vpm::SetVehicleStateRequest;
use sdvgenerated::sdv_vpm_power_notification_service_interface::PowerNotificationService;
use sdvgenerated::sdv_vpm_vpm_system_service_interface::VpmSystemService;
use sdvgenerated::vpm_system_client::VpmSystemClient;
use sdvgenerated::vpm_system_client::VpmSystemClientCallbacks;

use google_sdv_vpm_aidl::aidl::google::sdv::vpm::PowerStateReport::PowerStateReport as PowerStateReportAidl;
use google_sdv_vpm_aidl::binder;
use sdv_vpm_api::vpm::PowerStateNotificationRequest;
use sdv_vpm_api::vpm::PowerStateNotificationResponse;
use sdvmiddleware_rpc::RpcResult;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub const TIMEOUT_IN_SECS: Duration = Duration::from_secs(15);
pub struct PowerCallbackPreparer {
    // The callback receiver will wait 10 seconds.
    // if power state report is set.
    sleep_callback_if_power_state: Option<PowerStateReportAidl>,
    fail_callback: bool,
    callback_sender: Sender<PowerStateReportAidl>,
    callback_receiver: Receiver<PowerStateReportAidl>,
}

impl PowerCallbackPreparer {
    pub fn new() -> Self {
        let (callback_sender, callback_receiver): (
            Sender<PowerStateReportAidl>,
            Receiver<PowerStateReportAidl>,
        ) = channel();
        Self {
            sleep_callback_if_power_state: None,
            fail_callback: false,
            callback_sender,
            callback_receiver,
        }
    }
    pub fn fail_callback(&mut self) -> &mut PowerCallbackPreparer {
        self.fail_callback = true;
        self
    }

    pub fn with_sleep_on_state(
        &mut self,
        power_state_report: PowerStateReportAidl,
    ) -> &mut PowerCallbackPreparer {
        self.sleep_callback_if_power_state = Some(power_state_report);
        self
    }

    pub fn assert_callback_received(&self, power_state_report: PowerStateReportAidl) {
        let callback_result =
            self.callback_receiver.recv_timeout(TIMEOUT_IN_SECS).expect("Timeout reached");
        assert_eq!(callback_result, power_state_report);
    }

    pub fn setup_binder_callbacks(&self) -> PowerStateReportAidl {
        binder::ProcessState::start_thread_pool();
        let cloned_sender = self.callback_sender.clone();
        let test_sdv_agent = TestSdvAgent {
            callback_sender: cloned_sender,
            sleep_callback_if_power_state: self.sleep_callback_if_power_state,
            fail_callback: self.fail_callback,
        };
        let power_state_client_interface =
            get_power_state_client_interface().expect("Cannot find VPM client binder service");
        let power_report_listener =
            BnPowerStateReportListener::new_binder(test_sdv_agent, BinderFeatures::default());

        // Return current state received by the callback
        power_state_client_interface.subscribeToPowerStateReport(&power_report_listener).unwrap()
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
    fail_callback: bool,
}

impl binder::Interface for TestSdvAgent {}

impl IPowerStateReportListener for TestSdvAgent {
    fn onPowerStateReportChanged(&self, power_state: PowerStateReportAidl) -> binder::Result<()> {
        if self.sleep_callback_if_power_state.is_some()
            && self.sleep_callback_if_power_state.unwrap() == power_state
        {
            thread::sleep(Duration::from_secs(10));
            if self.fail_callback {
                return Err(binder::StatusCode::UNKNOWN_ERROR.into());
            }
        }
        self.callback_sender.send(power_state).unwrap();
        Ok(())
    }
}

pub struct TestVePSM {
    power_request_receiver: Receiver<PowerStateNotificationRequest>,
    vehicle_callback_receiver: Receiver<VpmVehicleState>,
    client: Arc<dyn VpmSystemService>,
    bundle: VpmSystemClient,
}

impl TestVePSM {
    pub fn new() -> TestVePSM {
        let (vehicle_request_sender, vehicle_callback_receiver): (
            Sender<VpmVehicleState>,
            Receiver<VpmVehicleState>,
        ) = channel();
        let (power_request_sender, power_request_receiver): (
            Sender<PowerStateNotificationRequest>,
            Receiver<PowerStateNotificationRequest>,
        ) = channel();

        let callback = move |message: Result<VehicleStateChange>| {
            vehicle_request_sender.send(message.unwrap().vehicle_state.unwrap()).unwrap();
        };
        let mut bundle = VpmSystemClient::new(VpmSystemClientCallbacks {
            sdv_vpm_vehicle_vehicle_state_change_cb: Box::new(callback),
        });
        bundle.start().unwrap();

        let client = bundle.get_sdv_vpm_vpm_system_service_client("VpmSystemServiceBundle");

        let power_notification_receiver =
            PowerNotificationReceiver { sender: power_request_sender };
        bundle
            .sdv_vpm_power_notification_service_server
            .register_service_interface(Arc::new(power_notification_receiver));

        Self { power_request_receiver, vehicle_callback_receiver, client, bundle }
    }

    pub fn send_power_state_request(&self, request: PowerStateChangeRequest) {
        self.client.set_power_state(request).unwrap();
    }

    pub fn send_vehicle_state_request(&self, request: SetVehicleStateRequest) {
        self.client.set_vehicle_state(request).unwrap();
    }

    pub fn assert_vehicle_state_received(&self, vehicle_state: VpmVehicleState) {
        let result =
            self.vehicle_callback_receiver.recv_timeout(TIMEOUT_IN_SECS).expect("Timeout reached");
        assert_eq!(result, vehicle_state);
    }

    pub fn assert_power_notification(
        &self,
        power_state_report: PowerStateReport,
        reason: PowerStateChangeReason,
    ) {
        let power_notification =
            self.power_request_receiver.recv_timeout(TIMEOUT_IN_SECS).expect("Timeout reached");
        assert_eq!(power_notification.power_state.unwrap(), power_state_report);
        assert_eq!(power_notification.reason.unwrap(), reason);
    }
}

struct PowerNotificationReceiver {
    sender: Sender<PowerStateNotificationRequest>,
}
impl PowerNotificationService for PowerNotificationReceiver {
    fn notify_power_state(
        &self,
        request: PowerStateNotificationRequest,
    ) -> RpcResult<PowerStateNotificationResponse> {
        self.sender.send(request).unwrap();
        Ok(PowerStateNotificationResponse { ..Default::default() })
    }
}
