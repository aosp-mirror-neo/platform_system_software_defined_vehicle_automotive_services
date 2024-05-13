// Copyright 2024 Google LLC
//! Vehicle and Power Mode Agent.

#![allow(unused)]
mod power_service;
mod power_state_notifier;
mod system_service;

use crate::power_service::PowerService;
use crate::power_service::PowerServiceImpl;
use crate::power_state_notifier::PowerStateNotifier;
use crate::power_state_notifier::PowerStateNotifierImpl;
use crate::system_service::VpmService;
use crate::system_service::VpmServiceImpl;

use log::{error, info};
use sdv_vpm_api::power::PowerStateReport;
use sdvgenerated::vpm_system_service_bundle::VpmSystemServiceBundle;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::mpsc::{channel, Receiver, Sender};

const POWER_STATE_CHANNEL_QUEUE_SIZE: usize = 1;

/// VPM agent.
/// The agent contains service implementation for vehicle and power mode management.
pub struct Agent {
    system_service: Arc<dyn VpmService>,
    power_service: Arc<Mutex<dyn PowerService>>,
    power_state_notifier: Box<dyn PowerStateNotifier>,
}

impl Default for Agent {
    fn default() -> Self {
        Self::new()
    }
}

/// VPM agent implementation.
impl Agent {
    /// Creates a new VPM agent.
    pub fn new() -> Self {
        let mut vpm_bundle = VpmSystemServiceBundle::new();
        vpm_bundle.start().unwrap();
        let (sender, receiver) = channel::<PowerStateReport>(POWER_STATE_CHANNEL_QUEUE_SIZE);
        let power_service = Arc::new(Mutex::new(PowerServiceImpl::new(sender)));
        let system_service = Arc::new(VpmServiceImpl::new(Arc::new(Mutex::new(vpm_bundle))));
        let power_state_notifier = Box::new(PowerStateNotifierImpl::new(receiver));
        Self::new_with_services(system_service, power_service, power_state_notifier)
    }

    fn new_with_services(
        system_service: Arc<dyn VpmService>,
        power_service: Arc<Mutex<dyn PowerService>>,
        power_state_notifier: Box<dyn PowerStateNotifier>,
    ) -> Self {
        Self { system_service, power_service, power_state_notifier }
    }

    /// Starts the VPM agent.
    /// The agent will start the vehicle state service and the power mode state service.
    pub async fn start(&mut self) {
        info!("Starting VPM Agent.");
        self.power_state_notifier.create_binder_service();
        let notifier_handler = self.power_state_notifier.start();
        let system_handler = self.system_service.start(self.power_service.clone());
        tokio::join!(notifier_handler, system_handler);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::power_service::MockPowerService;
    use crate::power_state_notifier::MockPowerStateNotifier;
    use crate::system_service::MockVpmService;
    use crate::Agent;
    use mockall::predicate;
    use tokio::runtime::Handle;
    use tokio::time::Duration;

    #[tokio::test]
    async fn vpm_agent_starts() {
        // Given: a mock services.
        let mut mock_vpm_service = MockVpmService::default();
        let mut mock_power_state_notifier = MockPowerStateNotifier::default();
        let mock_power_service = MockPowerService::default();

        // Expect: binder server for power state notifier has been created.
        mock_power_state_notifier.expect_create_binder_service().times(1).returning(|| Ok(()));
        // Expect: the publisher and the rpc server will be started.return_const(())
        mock_vpm_service.expect_start().times(1).return_const(());
        // Expect: the power state notifier to start
        mock_power_state_notifier.expect_start().times(1).returning(|| Ok(()));

        // When: the new agent is created, publisher and rpc servers are created.
        let mut agent = Agent::new_with_services(
            Arc::new(mock_vpm_service),
            Arc::new(Mutex::new(mock_power_service)),
            Box::new(mock_power_state_notifier),
        );
        let _test = agent.start().await;
    }
}
