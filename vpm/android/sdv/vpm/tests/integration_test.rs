// Copyright 2024 Google LLC

#[cfg(test)]
mod setup;
mod tests {
    use crate::setup::TestContext;
    use google_sdv_vpm_aidl::aidl::google::sdv::vpm::PowerStateReport::PowerStateReport as PowerStateReportAidl;
    use sdv_vpm_api::power::PowerStateReport;
    use sdv_vpm_api::vehicle::VpmVehicleState;
    use sdv_vpm_api::vpm::power_state_notification_request::PowerStateChangeReason;
    use sdv_vpm_api::vpm::PowerStateChangeRequest;
    use sdv_vpm_api::vpm::PowerStateRequest;
    use sdv_vpm_api::vpm::SetVehicleStateRequest;

    #[test]
    fn set_vehicle_states_and_power_states_notifies_system_clients() {
        // Given: a subscriber for vehicle state.
        let mut test_context = TestContext::new();

        // When: a new vehicle state is sent via RPC.
        let request = SetVehicleStateRequest {
            vehicle_state: VpmVehicleState::SOFTWARE_UPDATE.into(),
            ..Default::default()
        };
        test_context.vepsm.send_vehicle_state_request(request);

        // Then: the new vehicle state should be received by the subscriber.
        test_context.vepsm.assert_vehicle_state_received(VpmVehicleState::SOFTWARE_UPDATE);

        // Given power state SDV agents subscribers
        test_context.power_callback_preparer.setup_binder_callbacks();
        assert!(test_context.is_good_start_state());

        // When: the power state is sent via RPC from OEM.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::TURN_ON.into(),
            timeout_ms: 10000,
            ..Default::default()
        };
        test_context.vepsm.send_power_state_request(request);

        // Then: SDV agents can receive callbacks.
        test_context.power_callback_preparer.assert_callback_received(PowerStateReportAidl::ON);

        // Then: OEM receives confirmation about power state.
        test_context.vepsm.assert_power_notification(
            PowerStateReport::ON,
            PowerStateChangeReason::HOST_REQUESTED,
        );
    }
}
