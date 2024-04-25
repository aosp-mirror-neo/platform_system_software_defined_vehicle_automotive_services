use crate::RpcServer;
use async_trait::async_trait;
use log::{error, info};
use mockall::automock;
use sdv_vpm_api::vehicle::VehicleStateChange;
use sdv_vpm_api::vehicle::VpmVehicleState;
use sdv_vpm_api::vpm::SetVehicleStateRequest;
use sdvgenerated::vpm_system_service_bundle::VpmSystemServiceBundle;
use sdvmiddleware_rpc::error::RpcError;
use sdvmiddleware_rpc::RpcResult;
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::task;

#[automock]
#[async_trait]
// The trait encapsulates the functions of the service bundle.
// It also helps with testing the VPM agent while mocking the middleware generated code.
pub trait VpmService {
    // Starts the rpc server that receives incoming RPC requests from the OEM to set the vehicle state in the SDV VM.
    fn start_rpc_server(&self, rpc_server: RpcServer);

    // Starts the rpc handler which waits for RPC requests to set the vehicle state and publishes them into DT.
    async fn start_vehicle_state_handler(&self, vehicle_state_receiver: Receiver<VpmVehicleState>);
}

pub struct VpmServiceImpl {
    pub vpm_service_bundle: Arc<Mutex<VpmSystemServiceBundle>>,
}

#[async_trait]
impl VpmService for VpmServiceImpl {
    fn start_rpc_server(&self, rpc_server: RpcServer) {
        self.vpm_service_bundle
            .lock()
            .unwrap()
            .sdv_vpm_vpm_system_service_server
            .register_service_interface(Arc::new(rpc_server));
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
