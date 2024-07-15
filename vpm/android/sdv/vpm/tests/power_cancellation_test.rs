// Copyright 2024 Google LLC

#[cfg(test)]
mod common;
mod tests {
    use crate::common::PowerCallbackPreparer;
    use crate::common::TestVePSM;
    use google_sdv_vpm_aidl::aidl::google::sdv::vpm::PowerStateReport::PowerStateReport as PowerStateReportAidl;
    use sdv_vpm_api::power::PowerStateReport;
    use sdv_vpm_api::vpm::power_state_notification_request::PowerStateChangeReason;
    use sdv_vpm_api::vpm::PowerStateChangeRequest;
    use sdv_vpm_api::vpm::PowerStateRequest;
    use sdv_vpm_api::vpm::ShutdownType;
    use sdvgenerated::sdv_vpm_vpm_system_service_interface::VpmSystemService;
    use sdvgenerated::vpm_system_client::VpmSystemClient;
    use sdvgenerated::vpm_system_client::VpmSystemClientCallbacks;
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn cancel_power_state_prepare_request() {
        //Given A VM in Power On state.
        let test_bundle = VpmSystemClient::new(VpmSystemClientCallbacks {
            sdv_vpm_vehicle_vehicle_state_change_cb: Box::new(move |_| {}),
        });

        let (test_vepsm, request_receiver) =
            TestVePSM::create_vepsm_and_notification_receiver_channel();
        test_bundle
            .sdv_vpm_power_notification_service_server
            .register_service_interface(Arc::new(test_vepsm));

        let (current_power_state_report, callback_receiver) =
            PowerCallbackPreparer::new().setup_binder_callbacks();
        assert_eq!(current_power_state_report, PowerStateReportAidl::POWER_OFF_EXIT);

        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::TURN_ON.into(),
            timeout_ms: 10000,
            ..Default::default()
        };
        test_bundle
            .get_sdv_vpm_vpm_system_service_client("VpmSystemServiceBundle")
            .set_power_state(request)
            .unwrap();

        let callback_result =
            callback_receiver.recv_timeout(Duration::from_secs(5)).expect("Timeout reached");
        assert_eq!(callback_result, PowerStateReportAidl::ON);

        let power_notification =
            request_receiver.recv_timeout(Duration::from_secs(5)).expect("Timeout reached");
        assert_eq!(power_notification.power_state.unwrap(), PowerStateReport::ON);

        // When: the VM is on prepare state for ram suspension and a cancellation is requested.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::PREPARE_SHUTDOWN.into(),
            shutdown_type: ShutdownType::SUSPEND_TO_RAM.into(),
            timeout_ms: 10000,
            ..Default::default()
        };

        test_bundle
            .get_sdv_vpm_vpm_system_service_client("VpmSystemServiceBundle")
            .set_power_state(request)
            .unwrap();

        let callback_result =
            callback_receiver.recv_timeout(Duration::from_secs(5)).expect("Timeout reached");
        assert_eq!(callback_result, PowerStateReportAidl::SUSPEND_TO_RAM_ENTER);

        let power_notification =
            request_receiver.recv_timeout(Duration::from_secs(10)).expect("Timeout reached");
        assert_eq!(power_notification.power_state.unwrap(), PowerStateReport::SUSPEND_TO_RAM_ENTER);

        let callback_result =
            callback_receiver.recv_timeout(Duration::from_secs(5)).expect("Timeout reached");
        assert_eq!(callback_result, PowerStateReportAidl::WAIT_FOR_FINISH);

        let power_notification =
            request_receiver.recv_timeout(Duration::from_secs(10)).expect("Timeout reached");
        assert_eq!(power_notification.power_state.unwrap(), PowerStateReport::WAIT_FOR_FINISH);
        assert_eq!(power_notification.reason.unwrap(), PowerStateChangeReason::HOST_REQUESTED);

        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::CANCEL_SHUTDOWN.into(),
            timeout_ms: 10000,
            ..Default::default()
        };

        test_bundle
            .get_sdv_vpm_vpm_system_service_client("VpmSystemServiceBundle")
            .set_power_state(request)
            .unwrap();

        // Then: the vm is in shutdown cancelled state.
        let power_notification =
            request_receiver.recv_timeout(Duration::from_secs(10)).expect("Timeout reached");
        assert_eq!(power_notification.power_state.unwrap(), PowerStateReport::SHUTDOWN_CANCELLED);
        assert_eq!(power_notification.reason.unwrap(), PowerStateChangeReason::HOST_REQUESTED);
    }
}
