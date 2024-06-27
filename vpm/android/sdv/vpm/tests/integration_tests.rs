// Copyright 2024 Google LLC

#[cfg(test)]
mod tests {
    use crate::tests::binder::BinderFeatures;
    use crate::tests::binder::Strong;
    use google_sdv_vpm_aidl::aidl::google::sdv::vpm::IPowerStateClientApi::{
        BpPowerStateClientApi, IPowerStateClientApi,
    };
    use google_sdv_vpm_aidl::aidl::google::sdv::vpm::IPowerStateReportListener::{
        BnPowerStateReportListener, IPowerStateReportListener,
    };
    use google_sdv_vpm_aidl::aidl::google::sdv::vpm::PowerStateReport::PowerStateReport as PowerStateReportAidl;
    use google_sdv_vpm_aidl::binder;
    use sdv_middleware_pubsub::Result;
    use sdv_vpm_api::power::PowerStateReport;
    use sdv_vpm_api::vehicle::VehicleStateChange;
    use sdv_vpm_api::vehicle::VpmVehicleState;
    use sdv_vpm_api::vpm::PowerStateChangeRequest;
    use sdv_vpm_api::vpm::PowerStateNotificationRequest;
    use sdv_vpm_api::vpm::PowerStateNotificationResponse;
    use sdv_vpm_api::vpm::PowerStateRequest;
    use sdv_vpm_api::vpm::SetVehicleStateRequest;
    use sdvgenerated::sdv_vpm_power_notification_service_interface::PowerNotificationService;
    use sdvgenerated::sdv_vpm_vpm_system_service_interface::VpmSystemService;
    use sdvgenerated::vpm_system_client::VpmSystemClient;
    use sdvgenerated::vpm_system_client::VpmSystemClientCallbacks;
    use sdvmiddleware_rpc::RpcResult;
    use std::sync::mpsc::{channel, Receiver, Sender};
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn set_vehicle_states_notifies_system_clients() {
        let (tx, rx): (Sender<VpmVehicleState>, Receiver<VpmVehicleState>) = channel();

        // Given: a subscriber for vehicle state.
        let callback = move |message: Result<VehicleStateChange>| {
            tx.send(message.unwrap().vehicle_state.unwrap()).unwrap();
        };
        let mut test_bundle = VpmSystemClient::new(VpmSystemClientCallbacks {
            sdv_vpm_vehicle_vehicle_state_change_cb: Box::new(callback),
        });

        // When: a new vehicle state is sent via RPC.
        let request = SetVehicleStateRequest {
            vehicle_state: VpmVehicleState::SOFTWARE_UPDATE.into(),
            ..Default::default()
        };
        test_bundle
            .get_sdv_vpm_vpm_system_service_client("VpmSystemServiceBundle")
            .set_vehicle_state(request)
            .unwrap();
        test_bundle.start().unwrap();

        // Then: the new vehicle state should be received by the subscriber.
        let result = rx.recv_timeout(Duration::from_secs(5)).expect("Timeout reached");
        assert_eq!(result, VpmVehicleState::SOFTWARE_UPDATE);
    }

    #[test]
    fn set_power_state_notifies_power_report_listener_and_oem() {
        // Given: a power state.

        // Set up OEM power state client.
        let test_bundle = VpmSystemClient::new(VpmSystemClientCallbacks {
            sdv_vpm_vehicle_vehicle_state_change_cb: Box::new(move |_| {}),
        });
        let (request_sender, request_receiver): (
            Sender<PowerStateNotificationRequest>,
            Receiver<PowerStateNotificationRequest>,
        ) = channel();
        // Register OEM power state response server.
        test_bundle
            .sdv_vpm_power_notification_service_server
            .register_service_interface(Arc::new(TestVePSM { request_sender }));

        let (current_power_state_report, callback_receiver) =
            setup_binder_callbacks_for_power_state_listeners();
        assert_eq!(current_power_state_report, PowerStateReportAidl::POWER_OFF_EXIT);

        // When: the power state is sent via RPC from OEM.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::TURN_ON.into(),
            ..Default::default()
        };
        test_bundle
            .get_sdv_vpm_vpm_system_service_client("VpmSystemServiceBundle")
            .set_power_state(request)
            .unwrap();

        // Then: SDV agents can receive callbacks.
        let callback_result =
            callback_receiver.recv_timeout(Duration::from_secs(5)).expect("Timeout reached");
        assert_eq!(callback_result, PowerStateReportAidl::ON);

        // Then: OEM receives confirmation about power state.
        let power_notification =
            request_receiver.recv_timeout(Duration::from_secs(5)).expect("Timeout reached");
        assert_eq!(power_notification.power_state.unwrap(), PowerStateReport::ON);
    }

    fn setup_binder_callbacks_for_power_state_listeners(
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
        let current_power_state_report = power_state_client_interface
            .subscribeToPowerStateReport(&power_report_listener)
            .unwrap();
        (current_power_state_report, callback_receiver)
    }

    fn get_power_state_client_interface() -> anyhow::Result<Strong<dyn IPowerStateClientApi>> {
        Ok(binder::wait_for_interface::<dyn IPowerStateClientApi>(
            <BpPowerStateClientApi as IPowerStateClientApi>::get_descriptor(),
        )?)
    }
    struct TestSdvAgent {
        callback_sender: Sender<PowerStateReportAidl>,
    }

    impl binder::Interface for TestSdvAgent {}

    impl IPowerStateReportListener for TestSdvAgent {
        fn onPowerStateReportChanged(
            &self,
            power_state: PowerStateReportAidl,
        ) -> binder::Result<()> {
            self.callback_sender.send(power_state).unwrap();
            Ok(())
        }
    }

    struct TestVePSM {
        request_sender: Sender<PowerStateNotificationRequest>,
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
}
