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
    use sdvgenerated::sdv_vpm_vpm_system_service_interface::VpmSystemService;
    use sdvgenerated::vpm_system_client::VpmSystemClient;
    use sdvgenerated::vpm_system_client::VpmSystemClientCallbacks;
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn set_power_state_when_callback_timeouts_switches_state_with_timeout_reason() {
        // Given a VM in a start state (Power Off Exit).

        let test_bundle = VpmSystemClient::new(VpmSystemClientCallbacks {
            sdv_vpm_vehicle_vehicle_state_change_cb: Box::new(move |_| {}),
        });

        let (test_vepsm, request_receiver) =
            TestVePSM::create_vepsm_and_notification_receiver_channel();
        test_bundle
            .sdv_vpm_power_notification_service_server
            .register_service_interface(Arc::new(test_vepsm));

        let (current_power_state_report, _) = PowerCallbackPreparer::new()
            .with_sleep_on_state(PowerStateReportAidl::ON)
            .setup_binder_callbacks();

        assert_eq!(current_power_state_report, PowerStateReportAidl::POWER_OFF_EXIT);

        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::TURN_ON.into(),
            // setting the timeout to something very low (lower than the sleep value of the callback which is 10 secs) to guarantee failure.
            timeout_ms: 1,
            ..Default::default()
        };
        // When: attempting to transition to On state
        test_bundle
            .get_sdv_vpm_vpm_system_service_client("VpmSystemServiceBundle")
            .set_power_state(request)
            .unwrap();

        // Then: transition to On happens and timeout reason is returned.
        let power_notification =
            request_receiver.recv_timeout(Duration::from_secs(10)).expect("Timeout reached");
        assert_eq!(power_notification.power_state.unwrap(), PowerStateReport::ON);
        assert_eq!(power_notification.reason.unwrap(), PowerStateChangeReason::TIMEOUT);
    }
}
