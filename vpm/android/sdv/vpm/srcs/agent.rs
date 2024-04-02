// Copyright 2023 Google LLC
//! Vehicle and Power Mode Agent.

#![allow(unused)]
mod rpc_server;
mod service;

use crate::rpc_server::RpcServer;
use crate::service::VpmService;
use crate::service::VpmServiceImpl;
use log::{error, info};
use sdvgenerated::vpm_system_service_bundle::VpmSystemServiceBundle;
use std::sync::mpsc::channel;
use std::sync::mpsc::sync_channel;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::Mutex;

/// VPM agent.
/// The agent contains service implementation for vehicle and power mode management.
pub struct VPM {
    system_service: Arc<dyn VpmService>,
}

impl Default for VPM {
    fn default() -> Self {
        Self::new()
    }
}

/// VPM agent implementation.
impl VPM {
    /// Creates a new VPM agent.
    pub fn new() -> Self {
        let mut vpm_bundle = VpmSystemServiceBundle::new();
        vpm_bundle.start().unwrap();
        Self::new_with_services(Arc::new(VpmServiceImpl {
            vpm_service_bundle: Arc::new(Mutex::new(vpm_bundle)),
        }))
    }

    fn new_with_services(system_service: Arc<dyn VpmService>) -> Self {
        Self { system_service }
    }

    /// Starts the VPM agent.
    /// The agent will start the vehicle state service and the power mode state service.
    pub async fn start(&self) {
        info!("Starting VPM Agent.");
        let (vehicle_state_sender, vehicle_state_receiver) = channel();
        let rpc_server = RpcServer::new(vehicle_state_sender);
        self.system_service.start_rpc_server(rpc_server);
        self.system_service.start_vehicle_state_handler(vehicle_state_receiver).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc_server::RpcServer;
    use crate::service::MockVpmService;
    use crate::VPM;
    use mockall::predicate;
    use tokio::runtime::Handle;
    use tokio::time::Duration;

    #[tokio::test]
    async fn vpm_agent_starts() {
        // Given: a mock service.
        let mut mock_service = MockVpmService::default();

        // Expect: the publisher and the rpc server will be started.
        mock_service.expect_start_rpc_server().times(1).return_const(());
        mock_service.expect_start_vehicle_state_handler().times(1).return_const(());

        // When: the new agent is created, publisher and rpc servers are created.
        let agent = VPM::new_with_services(Arc::new(mock_service));
        let _test = agent.start().await;
    }
}
