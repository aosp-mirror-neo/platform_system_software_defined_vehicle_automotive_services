// Copyright 2024 Google LLC

#[cfg(test)]
mod common;
mod tests {
    use crate::common::PowerCallbackPreparer;
    use crate::common::TestVePSM;
    use google_sdv_vpm_aidl::aidl::google::sdv::vpm::PowerStateReport::PowerStateReport as PowerStateReportAidl;
    use sdv_middleware_pubsub::Result;
    use sdv_vpm_api::power::PowerStateReport;
    use sdv_vpm_api::vehicle::VehicleStateChange;
    use sdv_vpm_api::vehicle::VpmVehicleState;
    use sdv_vpm_api::vpm::power_state_notification_request::PowerStateChangeReason;
    use sdv_vpm_api::vpm::PowerStateChangeRequest;
    use sdv_vpm_api::vpm::PowerStateRequest;
    use sdv_vpm_api::vpm::SetVehicleStateRequest;
    use sdvgenerated::sdv_vpm_vpm_system_service_interface::VpmSystemService;
    use sdvgenerated::vpm_system_client::VpmSystemClient;
    use sdvgenerated::vpm_system_client::VpmSystemClientCallbacks;
    use std::sync::mpsc::{channel, Receiver, Sender};
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn set_vehicle_states_and_power_states_notifies_system_clients() {
        let (tx, rx): (Sender<VpmVehicleState>, Receiver<VpmVehicleState>) = channel();

        // Given: a subscriber for vehicle state.

        // Setup bundle
        let callback = move |message: Result<VehicleStateChange>| {
            tx.send(message.unwrap().vehicle_state.unwrap()).unwrap();
        };
        let mut test_bundle = VpmSystemClient::new(VpmSystemClientCallbacks {
            sdv_vpm_vehicle_vehicle_state_change_cb: Box::new(callback),
        });
        let (test_vepsm, request_receiver) =
            TestVePSM::create_vepsm_and_notification_receiver_channel();
        test_bundle
            .sdv_vpm_power_notification_service_server
            .register_service_interface(Arc::new(test_vepsm));

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

        // Given power state SDV agents subscribers

        let (current_power_state_report, callback_receiver) =
            PowerCallbackPreparer::new().setup_binder_callbacks();
        assert_eq!(current_power_state_report, PowerStateReportAidl::POWER_OFF_EXIT);

        // When: the power state is sent via RPC from OEM.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::TURN_ON.into(),
            timeout_ms: 10000,
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
            request_receiver.recv_timeout(Duration::from_secs(15)).expect("Timeout reached");
        assert_eq!(power_notification.power_state.unwrap(), PowerStateReport::ON);
        assert_eq!(power_notification.reason.unwrap(), PowerStateChangeReason::HOST_REQUESTED);
    }
}
