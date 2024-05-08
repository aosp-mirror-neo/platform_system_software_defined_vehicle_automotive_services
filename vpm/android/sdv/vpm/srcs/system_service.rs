// Copyright 2024 Google LLC
//! Vehicle and Power Mode Agent.

use async_trait::async_trait;
use log::{error, info};
use mockall::automock;
use protobuf::well_known_types::empty::Empty;
use sdv_vpm_api::vehicle::VehicleStateChange;
use sdv_vpm_api::vehicle::VpmVehicleState;
use sdv_vpm_api::vpm::PowerStateChangeRequest;
use sdv_vpm_api::vpm::PowerStateChangeResponse;
use sdv_vpm_api::vpm::SetVehicleStateRequest;
use sdvgenerated::sdv_vpm_vpm_system_service_interface::VpmSystemService;
use sdvgenerated::vpm_system_service_bundle::VpmSystemServiceBundle;
use sdvmiddleware_rpc::error::RpcError;
use sdvmiddleware_rpc::RpcResult;
use std::sync::mpsc::channel;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::task;

#[automock]
#[async_trait]
// The trait encapsulates the functions of the service bundle.
// It also helps with testing the VPM agent while mocking the middleware generated code.
pub trait VpmService: Send + Sync {
    // Starts the rpc server that receives incoming RPC requests from the OEM to set the vehicle and power state in the SDV VM.
    async fn start(&self);
}

pub struct VpmServiceImpl {
    vpm_service_bundle: Arc<Mutex<VpmSystemServiceBundle>>,
}

impl VpmServiceImpl {
    pub fn new(vpm_service_bundle: Arc<Mutex<VpmSystemServiceBundle>>) -> Self {
        Self { vpm_service_bundle }
    }
}

#[async_trait]
impl VpmService for VpmServiceImpl {
    async fn start(&self) {
        let (vehicle_state_sender, vehicle_state_receiver) = channel();
        let rpc_server = RpcServer::new(vehicle_state_sender);

        // Register GRPC interface for Vehicle states
        self.vpm_service_bundle
            .lock()
            .unwrap()
            .sdv_vpm_vpm_system_service_server
            .register_service_interface(Arc::new(rpc_server));

        // Start a blocking thread to listen to incoming vehicle mode changes
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
                        .sdv_vpm_vehicle_vehicle_state_change_publish(&vehicle_state_change_message)
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

/// A listener struct that handles incoming RPC requests for setting the vpm.
struct RpcServer {
    vehicle_state_sender: Sender<VpmVehicleState>,
}

impl RpcServer {
    pub fn new(vehicle_state_sender: Sender<VpmVehicleState>) -> Self {
        Self { vehicle_state_sender }
    }
}

/// Middleware RPC server trait implementation for RPC server.
impl VpmSystemService for RpcServer {
    fn set_vehicle_state(&self, request: SetVehicleStateRequest) -> RpcResult<Empty, RpcError> {
        info!("Got Request! {}", request);
        self.vehicle_state_sender.send(request.vehicle_state.unwrap()).unwrap();
        Ok(().into())
    }
    fn set_power_state(
        &self,
        request: PowerStateChangeRequest,
    ) -> RpcResult<PowerStateChangeResponse, RpcError> {
        //TODO:: implement set power state
        Ok(PowerStateChangeResponse::new())
    }
}
