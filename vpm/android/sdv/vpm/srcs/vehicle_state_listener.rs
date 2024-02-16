// Copyright 2023 Google LLC
//! Vehicle Mode and Power Management Agent.

use log::info;
use protobuf::well_known_types::empty::Empty;
use sdv_vpm_api::vehicle::VpmVehicleState;
use sdv_vpm_api::vpm::SetVehicleStateRequest;
use sdvgenerated::vpm_system_service_interface::VpmSystemService;
use sdvmiddleware_rpc::error::RpcError;
use sdvmiddleware_rpc::RpcResult;
use std::sync::mpsc::Sender;

/// A listener struct that handles incoming RPC requests for setting the vehicle state.
pub struct VehicleStateListener {
    vehicle_state_sender: Sender<VpmVehicleState>,
}

impl VehicleStateListener {
    pub fn new(vehicle_state_sender: Sender<VpmVehicleState>) -> Self {
        Self { vehicle_state_sender }
    }
}

/// Middleware RPC server trait implementation for RPC server.
impl VpmSystemService for VehicleStateListener {
    fn set_vehicle_state(&self, request: SetVehicleStateRequest) -> RpcResult<Empty, RpcError> {
        info!("Got Request! {}", request);
        self.vehicle_state_sender.send(request.vehicle_state.unwrap()).unwrap();
        Ok(().into())
    }
}
