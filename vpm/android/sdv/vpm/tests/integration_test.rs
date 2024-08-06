// Copyright 2024 Google LLC

#[cfg(test)]
mod common;
mod tests {
    use crate::common::PowerCallbackPreparer;
    use crate::common::TestVePSM;
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
        // Setup VePSM
        let test_vepsm = TestVePSM::new();

        // When: a new vehicle state is sent via RPC.
        let request = SetVehicleStateRequest {
            vehicle_state: VpmVehicleState::SOFTWARE_UPDATE.into(),
            ..Default::default()
        };
        test_vepsm.send_vehicle_state_request(request);

        // Then: the new vehicle state should be received by the subscriber.
        test_vepsm.assert_vehicle_state_received(VpmVehicleState::SOFTWARE_UPDATE);

        // Given power state SDV agents subscribers
        let power_callback_preparer = PowerCallbackPreparer::new();
        let current_power_state_report = power_callback_preparer.setup_binder_callbacks();
        assert_eq!(current_power_state_report, PowerStateReportAidl::POWER_OFF_EXIT);

        // When: the power state is sent via RPC from OEM.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::TURN_ON.into(),
            timeout_ms: 10000,
            ..Default::default()
        };
        test_vepsm.send_power_state_request(request);

        // Then: SDV agents can receive callbacks.
        power_callback_preparer.assert_callback_received(PowerStateReportAidl::ON);

        // Then: OEM receives confirmation about power state.
        test_vepsm.assert_power_notification(
            PowerStateReport::ON,
            PowerStateChangeReason::HOST_REQUESTED,
        );
    }
}
