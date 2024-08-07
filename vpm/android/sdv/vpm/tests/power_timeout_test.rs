// Copyright 2024 Google LLC

#[cfg(test)]
mod setup;
mod tests {
    use crate::setup::TestContext;
    use google_sdv_vpm_aidl::aidl::google::sdv::vpm::PowerStateReport::PowerStateReport as PowerStateReportAidl;
    use sdv_vpm_api::power::PowerStateReport;
    use sdv_vpm_api::vpm::power_state_notification_request::PowerStateChangeReason;
    use sdv_vpm_api::vpm::PowerStateChangeRequest;
    use sdv_vpm_api::vpm::PowerStateRequest;

    #[test]
    fn set_power_state_when_callback_timeouts_switches_state_with_timeout_reason() {
        // Given a VM in a start state (Power Off Exit).
        let mut test_context = TestContext::new();
        test_context
            .power_callback_preparer
            .with_sleep_on_state(PowerStateReportAidl::ON)
            .setup_binder_callbacks();
        assert!(test_context.is_good_start_state());

        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::TURN_ON.into(),
            // setting the timeout to something very low (lower than the sleep value of the callback which is 10 secs) to guarantee failure.
            timeout_ms: 1,
            ..Default::default()
        };
        // When: attempting to transition to On state
        test_context.vepsm.send_power_state_request(request);

        // Then: transition to On happens and timeout reason is returned.
        test_context
            .vepsm
            .assert_power_notification(PowerStateReport::ON, PowerStateChangeReason::TIMEOUT);
    }
}
