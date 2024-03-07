#[cfg(test)]
mod tests {
    use sdv_middleware_pubsub::Result;
    use sdv_vpm_api::vehicle::VehicleStateChange;
    use sdv_vpm_api::vehicle::VpmVehicleState;
    use sdv_vpm_api::vpm::SetVehicleStateRequest;
    use sdvgenerated::vpm_integration_test_bundle::VpmIntegrationTestBundle;
    use sdvgenerated::vpm_integration_test_bundle::VpmIntegrationTestBundleCallbacks;
    use sdvgenerated::vpm_system_service_interface::VpmSystemService;
    use std::sync::mpsc::{channel, Receiver, Sender};
    use std::time::Duration;

    #[test]
    fn vpm_integration_test() {
        let (tx, rx): (Sender<VpmVehicleState>, Receiver<VpmVehicleState>) = channel();

        // Given a subscriber for vehicle state.
        let callback = move |message: Result<VehicleStateChange>| {
            tx.send(message.unwrap().vehicle_state.unwrap()).unwrap();
        };
        let mut test_bundle = VpmIntegrationTestBundle::new(VpmIntegrationTestBundleCallbacks {
            vehicle_state_change_cb: Box::new(callback),
        });

        // When a new vehicle state is sent via RPC.
        let request = SetVehicleStateRequest {
            vehicle_state: VpmVehicleState::SOFTWARE_UPDATE.into(),
            ..Default::default()
        };
        test_bundle
            .get_vpm_system_service_client("VpmSystemServiceBundle")
            .set_vehicle_state(request)
            .unwrap();
        test_bundle.start().unwrap();

        // Then then new vehicle state should be received by the subscriber.
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(value) => assert_eq!(value, VpmVehicleState::SOFTWARE_UPDATE),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => panic!("Timeout reached"),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => panic!("Channel disconnected"),
        }
    }
}
