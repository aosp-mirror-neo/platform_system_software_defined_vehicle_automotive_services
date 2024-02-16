// Copyright 2023 Google LLC
//! Vehicle and Power Mode Agent.

#![allow(unused)]
use crate::vehicle_state_listener::VehicleStateListener;
use log::{error, info};
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
use std::sync::Arc;
use std::sync::Mutex;
use tokio::task;

/// VPM agent.
/// The agent contains service implementation for vehilce and power mode management.
pub struct VPM {
    vpm_system_bundle: Arc<Mutex<VpmSystemServiceBundle>>,
}

/// VPM agent implementation.
impl VPM {
    /// Creates a new VPM agent.
    pub fn new() -> Self {
        info!("Starting VPM vehicle service.");
        Self { vpm_system_bundle: Arc::new(Mutex::new(VpmSystemServiceBundle::new())) }
    }

    /// Starts a blocking thread to keep listeneing to incoming RPC requests.
    /// Registers vehicle state publishing topics.
    pub async fn start(&mut self) {
        let (vehicle_state_sender, vehicle_state_receiver) = channel();

        self.vpm_system_bundle
            .lock()
            .unwrap()
            .vpm_system_service_server
            .register_service_interface(Arc::new(VehicleStateListener::new(vehicle_state_sender)));

        self.vpm_system_bundle.lock().unwrap().start();

        let mut cloned_bundle = self.vpm_system_bundle.clone();
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
