// Copyright 2024 Google LLC
//! Vehicle and Power Mode Agent.

mod power_service;
mod power_state;
mod power_state_notifier;
mod system_service;
use crate::power_service::PowerService;
use crate::power_service::PowerServiceImpl;
use crate::power_service::PowerStateRequestHandlerImpl;
use crate::power_state::PowerState;
use crate::power_state_notifier::PowerStateNotifier;
use crate::power_state_notifier::PowerStateNotifierImpl;
use crate::system_service::VpmService;
use crate::system_service::VpmServiceImpl;
use log::info;
use mockall::automock;
use sdv_comms::id::ServiceFqin;
use sdv_comms::ContextRef;
use sdv_mw_rs_comms::Publisher;
use sdv_mw_rs_sdv_comms::SdvComms;
use sdv_mw_rs_sdv_vpm_vpm_system_service_bundle::publisher::Variant;
use sdv_mw_rs_sdv_vpm_vpm_system_service_bundle::VpmSystemServiceBundle;
use sdv_vpm_api::vehicle::VehicleStateChange;
use std::str::FromStr;
use std::sync::Arc;

const POWER_STATE_CHANNEL_QUEUE_SIZE: usize = 1;

const FQIN: &str = "vm:sdv.vpm.VpmSystemServiceBundle/default";

/// VPM agent.
/// The agent contains service implementation for vehicle and power mode management.
pub struct Agent {
    system_service: Box<dyn VpmService>,
    power_service: Box<dyn PowerService>,
    vpm_bundle: Box<dyn VpmServiceBundle>,
}

#[automock]
trait VpmServiceBundle: Send {
    fn get_vehicle_state_publisher(&mut self) -> Option<Publisher<VehicleStateChange>>;
}

impl VpmServiceBundle for VpmSystemServiceBundle {
    fn get_vehicle_state_publisher(&mut self) -> Option<Publisher<VehicleStateChange>> {
        self.take_publisher(Variant::UNIQUE)
    }
}

/// VPM agent implementation.
impl Agent {
    /// Creates a new VPM agent.
    pub async fn new() -> Self {
        let service_fqin = ServiceFqin::from_str(FQIN).expect("Could not create context for VPM!");
        let context = ContextRef::create(service_fqin.clone());
        let sdv_comms = Arc::new(SdvComms { context });
        let power_service = Box::new(PowerServiceImpl::new(sdv_comms.clone()));
        let (system_service, rpc_server) = VpmServiceImpl::new(
            power_service.get_power_state_request_sender(),
            power_service.get_power_state_watch_receiver(),
        );
        let vpm_bundle = VpmSystemServiceBundle::new(sdv_comms, (Arc::new(rpc_server),)).await;
        Self::new_with_services(
            Box::new(system_service),
            power_service,
            Box::new(vpm_bundle.expect("VPM Bundle failed to initialize")),
        )
    }

    fn new_with_services(
        system_service: Box<dyn VpmService>,
        power_service: Box<dyn PowerService>,
        vpm_bundle: Box<dyn VpmServiceBundle>,
    ) -> Self {
        Self { system_service, power_service, vpm_bundle }
    }

    /// Starts the VPM agent.
    /// The agent will start the vehicle state service and the power mode state service.
    pub async fn start(&mut self) {
        info!("Starting VPM Agent.");
        let _ = tokio::join!(
            self.system_service.start(self.vpm_bundle.get_vehicle_state_publisher()),
            self.power_service.start()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::power_service::MockPowerService;
    use crate::system_service::MockVpmService;
    use crate::Agent;

    #[tokio::test]
    async fn vpm_agent_starts() {
        // Given: a mock services.
        let mut mock_vpm_service = MockVpmService::default();
        let mut mock_power_service = MockPowerService::default();
        let mut mock_vpm_bundle = MockVpmServiceBundle::default();

        // Expect: the publisher and the rpc server will be started.return_const(()).
        mock_vpm_service.expect_start().times(1).return_const(());
        // Expect: the power service to start.
        mock_power_service.expect_start().times(1).returning(|| Ok(()));
        // Expect: get vehicle state publisher to be called.
        mock_vpm_bundle.expect_get_vehicle_state_publisher().times(1).returning(|| None);

        // When: the new agent is created, publisher and rpc servers are created.
        let mut agent = Agent::new_with_services(
            Box::new(mock_vpm_service),
            Box::new(mock_power_service),
            Box::new(mock_vpm_bundle),
        );

        let _test = agent.start().await;
    }
}
