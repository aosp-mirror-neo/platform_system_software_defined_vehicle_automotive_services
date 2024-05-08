// Copyright 2024 Google LLC
//! Vehicle and Power Mode Agent.

#![allow(unused)]
mod power_state_notifier;
mod system_service;

use crate::power_state_notifier::PowerStateNotifier;
use crate::power_state_notifier::PowerStateNotifierImpl;
use crate::system_service::VpmService;
use crate::system_service::VpmServiceImpl;
use log::{error, info};
use sdvgenerated::vpm_system_service_bundle::VpmSystemServiceBundle;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::Mutex;

/// VPM agent.
/// The agent contains service implementation for vehicle and power mode management.
pub struct Agent {
    system_service: Arc<dyn VpmService>,
    power_state_notifier: Arc<dyn PowerStateNotifier>,
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
        let system_service = VpmServiceImpl::new(Arc::new(Mutex::new(vpm_bundle)));
        let power_state_notifier = PowerStateNotifierImpl::new();
        Self::new_with_services(Arc::new(system_service), Arc::new(power_state_notifier))
    }

    fn new_with_services(
        system_service: Arc<dyn VpmService>,
        power_state_notifier: Arc<dyn PowerStateNotifier>,
    ) -> Self {
        Self { system_service, power_state_notifier }
    }

    /// Starts the VPM agent.
    /// The agent will start the vehicle state service and the power mode state service.
    pub async fn start(&self) {
        info!("Starting VPM Agent.");
        self.power_state_notifier.create_binder_service();
        self.system_service.start().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

        // Expect: binder server for power state notifier has been created.
        mock_power_state_notifier.expect_create_binder_service().times(1).returning(|| Ok(()));
        // Expect: the publisher and the rpc server will be started.return_const(())
        mock_vpm_service.expect_start().times(1).return_const(());

        // When: the new agent is created, publisher and rpc servers are created.
        let agent = Agent::new_with_services(
            Arc::new(mock_vpm_service),
            Arc::new(mock_power_state_notifier),
        );
        let _test = agent.start().await;
    }
}
