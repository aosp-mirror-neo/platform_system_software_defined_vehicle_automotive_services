// Copyright 2023 Google LLC
//! Vehicle and Power Mode Agent.

#![allow(unused)]
mod vehicle_state_listener;

use crate::vehicle_state_listener::VehicleStateListener;
use async_trait::async_trait;
use log::{error, info};
use mockall::automock;
use protobuf::well_known_types::empty::Empty;
use sdv_vpm_api::vehicle::VehicleStateChange;
use sdv_vpm_api::vehicle::VpmVehicleState;
use sdv_vpm_api::vpm::SetVehicleStateRequest;
use sdvgenerated::vpm_system_service_bundle::VpmSystemServiceBundle;
use sdvgenerated::vpm_system_service_interface::VpmSystemService;
use sdvmiddleware_rpc::error::RpcError;
use sdvmiddleware_rpc::RpcResult;
use std::sync::mpsc::channel;
use std::sync::mpsc::sync_channel;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::task;

#[automock]
#[async_trait]
// The trait encapsulates the functions of the vehicle states.
// It also helps with testing the VPM agent while mocking the middleware generated code.
trait VehicleStateService {
    // Starts the rpc server that receives incoming RPC requests from the OEM to set the vehicle state in the SDV VM.
    fn start_rpc_server(&self, vpm_state_listener: VehicleStateListener);

    // Starts the rpc handler which waits for RPC requests to set the vehicle state and publishes them into DT.
    async fn start_vehicle_state_handler(&self, vehicle_state_receiver: Receiver<VpmVehicleState>);
}

struct VehicleStateServiceImpl {
    vpm_service_bundle: Arc<Mutex<VpmSystemServiceBundle>>,
}

#[async_trait]
impl VehicleStateService for VehicleStateServiceImpl {
    fn start_rpc_server(&self, vehicle_state_listener: VehicleStateListener) {
        self.vpm_service_bundle
            .lock()
            .unwrap()
            .vpm_system_service_server
            .register_service_interface(Arc::new(vehicle_state_listener));
    }

    async fn start_vehicle_state_handler(&self, vehicle_state_receiver: Receiver<VpmVehicleState>) {
        let cloned_bundle = self.vpm_service_bundle.clone();
        task::spawn_blocking(move || loop {
            match vehicle_state_receiver.recv() {
                Ok(vehicle_state) => {
                    let mut vehicle_state_change_message = VehicleStateChange::new();
                    vehicle_state_change_message.vehicle_state = vehicle_state.into();
                    info!("Publishing Vpm vehicle mode, {:?}!", vehicle_state);
                    cloned_bundle
                        .lock()
                        .unwrap()
                        .vehicle_state_change_publish(&vehicle_state_change_message)
                        .unwrap();
                }
                Err(err) => {
                    error!("Channel error: {}", err);
                    break;
                }
            }
        })
        .await;
    }
}

/// VPM agent.
/// The agent contains service implementation for vehicle and power mode management.
pub struct VPM {
    vehicle_service: Arc<dyn VehicleStateService>,
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
        Self::new_with_services(Arc::new(VehicleStateServiceImpl {
            vpm_service_bundle: Arc::new(Mutex::new(vpm_bundle)),
        }))
    }

    fn new_with_services(vehicle_service: Arc<dyn VehicleStateService>) -> Self {
        Self { vehicle_service }
    }

    /// Starts the VPM agent.
    /// The agent will start the vehilce state service and the power mode state service.
    pub async fn start(&self) {
        info!("Starting VPM Agent.");
        let (vehicle_state_sender, vehicle_state_receiver) = channel();
        self.vehicle_service.start_rpc_server(VehicleStateListener::new(vehicle_state_sender));
        self.vehicle_service.start_vehicle_state_handler(vehicle_state_receiver).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vehicle_state_listener::VehicleStateListener;
    use crate::VPM;
    use mockall::predicate;
    use tokio::runtime::Handle;
    use tokio::time::Duration;

    #[tokio::test]
    async fn vpm_agent_starts() {
        // Given: a mock service.
        let mut mock_service = MockVehicleStateService::default();

        // Expect: the publisher and the rpc server will be started.
        mock_service.expect_start_rpc_server().times(1).return_const(());
        mock_service.expect_start_vehicle_state_handler().times(1).return_const(());

        // When: the new agent is created, publisher and rpc servers are created.
        let agent = VPM::new_with_services(Arc::new(mock_service));
        let _test = agent.start().await;
    }
}
