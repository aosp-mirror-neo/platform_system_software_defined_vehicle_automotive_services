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

    #[test]
    fn interrupt_power_state_no_oem_response_sent() {
        // Given A VM in Power On state.
        // Setup VePSM
        let test_vepsm = TestVePSM::new();

        // We simulate that it takes too long to execute WaitForFinish
        // Add a sleep time in WaitForFinish in the client to simulate its taking too long.
        let mut power_callback_preparer = PowerCallbackPreparer::new();
        let current_power_state_report = power_callback_preparer
            .with_sleep_on_state(PowerStateReportAidl::WAIT_FOR_FINISH)
            .fail_callback()
            .setup_binder_callbacks();
        assert_eq!(current_power_state_report, PowerStateReportAidl::POWER_OFF_EXIT);

        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::TURN_ON.into(),
            timeout_ms: 10000,
            ..Default::default()
        };

        test_vepsm.send_power_state_request(request);

        power_callback_preparer.assert_callback_received(PowerStateReportAidl::ON);
        test_vepsm.assert_power_notification(
            PowerStateReport::ON,
            PowerStateChangeReason::HOST_REQUESTED,
        );

        // When: the VM is on prepare state for ram suspension and a cancellation is requested
        // in the middle of execution of WaitForFinish.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::PREPARE_SHUTDOWN.into(),
            shutdown_type: ShutdownType::SUSPEND_TO_RAM.into(),
            timeout_ms: 10000,
            ..Default::default()
        };

        test_vepsm.send_power_state_request(request);

        power_callback_preparer
            .assert_callback_received(PowerStateReportAidl::SUSPEND_TO_RAM_ENTER);
        test_vepsm.assert_power_notification(
            PowerStateReport::SUSPEND_TO_RAM_ENTER,
            PowerStateChangeReason::HOST_REQUESTED,
        );

        //When: Cancel shutdown in middle of wait for finish.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::CANCEL_SHUTDOWN.into(),
            timeout_ms: 10000,
            ..Default::default()
        };

        test_vepsm.send_power_state_request(request);

        // Then: the vm is in shutdown cancelled state.
        // State WaitForFinish is never received by the OEM.
        // The state might take more time than usual because it waits for the previous state to be cancelled.
        test_vepsm.assert_power_notification(
            PowerStateReport::SHUTDOWN_CANCELLED,
            PowerStateChangeReason::HOST_REQUESTED,
        );
    }
}
