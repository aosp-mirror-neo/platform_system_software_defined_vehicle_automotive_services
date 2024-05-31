// Copyright 2024 Google LLC
//! Vehicle and Power Mode Agent.
use crate::PowerService;
use async_trait::async_trait;
use log::{error, info};
use mockall::automock;
use protobuf::well_known_types::empty::Empty;
use sdv_vpm_api::vehicle::VehicleStateChange;
use sdv_vpm_api::vehicle::VpmVehicleState;
use sdv_vpm_api::vpm::power_state_change_response::PowerStateChangeRequestError;
use sdv_vpm_api::vpm::PowerStateChangeRequest;
use sdv_vpm_api::vpm::PowerStateChangeResponse;
use sdv_vpm_api::vpm::SetVehicleStateRequest;
use sdv_vpm_api::vpm::SetVehicleStateResponse;
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
    async fn start(&self, power_service: Arc<Mutex<dyn PowerService>>);
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
    async fn start(&self, power_service: Arc<Mutex<dyn PowerService>>) {
        let (vehicle_state_sender, vehicle_state_receiver) = channel();
        let rpc_server = RpcServer::new(vehicle_state_sender, power_service);

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
    power_service: Arc<Mutex<dyn PowerService>>,
}

impl RpcServer {
    pub fn new(
        vehicle_state_sender: Sender<VpmVehicleState>,
        power_service: Arc<Mutex<dyn PowerService>>,
    ) -> Self {
        Self { vehicle_state_sender, power_service }
    }
}

/// Middleware RPC server trait implementation for RPC server.
impl VpmSystemService for RpcServer {
    fn set_vehicle_state(
        &self,
        request: SetVehicleStateRequest,
    ) -> RpcResult<SetVehicleStateResponse, RpcError> {
        info!("Received Vehicle state change request! {}", request);
        let state = request.vehicle_state.enum_value_or_default();
        if state == VpmVehicleState::VEHICLE_STATE_UNSPECIFIED {
            return Err(RpcError::InvalidRequest);
        }
        let result = self.vehicle_state_sender.send(state);
        if result.is_err() {
            error!("Could not publish vehicle state!");
            return Err(RpcError::Other);
        }
        Ok(SetVehicleStateResponse::new())
    }

    fn set_power_state(
        &self,
        request: PowerStateChangeRequest,
    ) -> RpcResult<PowerStateChangeResponse, RpcError> {
        let result = self.power_service.lock().unwrap().set_power_state(request);
        if result.is_ok() {
            let mut response = PowerStateChangeResponse::new();
            response.error = PowerStateChangeRequestError::NO_ERROR.into();
            Ok(response)
        } else {
            let mut response = PowerStateChangeResponse::new();
            response.error = PowerStateChangeRequestError::INVALID_TRANSITION.into();
            Ok(response)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::power_service::MockPowerService;
    use crate::system_service::RpcServer;
    use anyhow::anyhow;
    use googletest::prelude::{assert_that, eq, matches_pattern};
    use sdv_vpm_api::vehicle::VpmVehicleState;
    use sdv_vpm_api::vpm::power_state_change_response::PowerStateChangeRequestError;
    use sdv_vpm_api::vpm::PowerStateChangeRequest;
    use sdv_vpm_api::vpm::PowerStateChangeResponse;
    use sdv_vpm_api::vpm::PowerStateRequest;
    use sdv_vpm_api::vpm::SetVehicleStateRequest;
    use sdv_vpm_api::vpm::ShutdownType;
    use sdvgenerated::sdv_vpm_vpm_system_service_interface::VpmSystemService;
    use sdvmiddleware_rpc::error::RpcError;
    use std::sync::mpsc::channel;
    use std::sync::Arc;
    use std::sync::Mutex;

    #[tokio::test]
    async fn set_rpc_vehicle_state() {
        // Given: mock services.
        let (vehicle_state_sender, vehicle_state_receiver) = channel();
        let mock_power_service = MockPowerService::default();
        let mut rpc_server =
            RpcServer::new(vehicle_state_sender, Arc::new(Mutex::new(mock_power_service)));
        let vehicle_state_handle = tokio::spawn(async move { vehicle_state_receiver.recv() });

        // When: a vehicle state request is sent
        let request = SetVehicleStateRequest {
            vehicle_state: VpmVehicleState::VEHICLE_ON.into(),
            ..Default::default()
        };
        let result = rpc_server.set_vehicle_state(request);

        // Then: vehicle state should be received by the channel.
        let expected_vehicle_state = vehicle_state_handle.await.unwrap();
        assert_eq!(
            VpmVehicleState::VEHICLE_ON,
            expected_vehicle_state.expect("Vehicle state should be sent over the channel")
        );
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn set_rpc_vehicle_state_fails_request_invalid() {
        // Given: mock services.
        let (vehicle_state_sender, vehicle_state_receiver) = channel();
        let mock_power_service = MockPowerService::default();
        let mut rpc_server =
            RpcServer::new(vehicle_state_sender, Arc::new(Mutex::new(mock_power_service)));
        let vehicle_state_handle = tokio::spawn(async move { vehicle_state_receiver.recv() });

        // When: a vehicle state request is sent without proper state.
        let request = SetVehicleStateRequest { ..Default::default() };

        // Then: result should be error.
        assert_that!(
            rpc_server.set_vehicle_state(request),
            matches_pattern!(Err(matches_pattern!(RpcError::InvalidRequest)))
        );
    }

    #[tokio::test]
    async fn set_rpc_vehicle_state_fails_no_receiver() {
        // Given: mock services without a receiver.
        let (vehicle_state_sender, vehicle_state_receiver) = channel();
        let mock_power_service = MockPowerService::default();
        let mut rpc_server =
            RpcServer::new(vehicle_state_sender, Arc::new(Mutex::new(mock_power_service)));

        // When: a vehicle state request is sent and receiver is dropped.
        let request = SetVehicleStateRequest {
            vehicle_state: VpmVehicleState::VEHICLE_ON.into(),
            ..Default::default()
        };
        drop(vehicle_state_receiver);

        // Then: result should be error.
        assert_that!(
            rpc_server.set_vehicle_state(request),
            matches_pattern!(Err(matches_pattern!(RpcError::Other)))
        );
    }

    #[test]
    fn set_rpc_power_state_succeeds() {
        // Given: a mock power service.
        let (vehicle_state_sender, vehicle_state_receiver) = channel();
        let mut mock_power_service = MockPowerService::default();
        mock_power_service.expect_set_power_state().times(1).returning(|_| Ok(()));
        let mut rpc_server =
            RpcServer::new(vehicle_state_sender, Arc::new(Mutex::new(mock_power_service)));

        // When: a power state request is sent
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::FINISH_SHUTDOWN.into(),
            shutdown_type: ShutdownType::POWER_OFF.into(),
            ..Default::default()
        };
        let response = rpc_server.set_power_state(request);

        // Then: power state should be set in the power service.
        assert_eq!(response.unwrap().error, PowerStateChangeRequestError::NO_ERROR.into());
    }

    #[test]
    fn set_rpc_power_state_fails() {
        // Given: a mock power service.
        let (vehicle_state_sender, vehicle_state_receiver) = channel();
        let mut mock_power_service = MockPowerService::default();
        mock_power_service
            .expect_set_power_state()
            .times(1)
            .returning(|_| Err(anyhow!("Power service failed")));
        let mut rpc_server =
            RpcServer::new(vehicle_state_sender, Arc::new(Mutex::new(mock_power_service)));

        // When: a power state request is sent but fails.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::FINISH_SHUTDOWN.into(),
            shutdown_type: ShutdownType::POWER_OFF.into(),
            ..Default::default()
        };
        let response = rpc_server.set_power_state(request);

        // Then: power state should not be set in the power service.
        // Failure is reported.
        assert_eq!(
            response.unwrap().error,
            PowerStateChangeRequestError::INVALID_TRANSITION.into()
        );
    }
}
