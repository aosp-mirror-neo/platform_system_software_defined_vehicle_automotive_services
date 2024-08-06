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

    #[test]
    fn set_power_state_when_callback_timeouts_switches_state_with_timeout_reason() {
        // Given a VM in a start state (Power Off Exit).

        let test_vepsm = TestVePSM::new();
        let power_callback_preparer = PowerCallbackPreparer::new();
        let current_power_state_report = power_callback_preparer.setup_binder_callbacks();
        assert_eq!(current_power_state_report, PowerStateReportAidl::POWER_OFF_EXIT);

        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::TURN_ON.into(),
            // setting the timeout to something very low (lower than the sleep value of the callback which is 10 secs) to guarantee failure.
            timeout_ms: 1,
            ..Default::default()
        };
        // When: attempting to transition to On state
        test_vepsm.send_power_state_request(request);

        // Then: transition to On happens and timeout reason is returned.
        test_vepsm.assert_power_notification(PowerStateReport::ON, PowerStateChangeReason::TIMEOUT);
    }
}
