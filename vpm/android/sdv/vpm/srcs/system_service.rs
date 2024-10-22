// Copyright 2024 Google LLC
//! Vehicle and Power Mode Agent.
//!
use crate::power_service::PowerRequest;
use crate::power_service::PowerStateRequestHandler;
use crate::PowerState;
use crate::PowerStateRequestHandlerImpl;
use async_trait::async_trait;
use log::{error, info};
use mockall::automock;
use sdv_mw_rs_comms::Publisher;
use sdv_status_rs::SdvResult;
use sdv_status_rs::SdvStatus;
use sdv_status_rs::SdvStatusCode;
use sdv_vpm_api::vehicle::VehicleStateChange;
use sdv_vpm_api::vehicle::VpmVehicleState;
use sdv_vpm_api::vpm::power_state_change_response::PowerStateChangeRequestError;
use sdv_vpm_api::vpm::PowerStateChangeRequest;
use sdv_vpm_api::vpm::PowerStateChangeResponse;
use sdv_vpm_api::vpm::SetVehicleStateRequest;
use sdv_vpm_api::vpm::SetVehicleStateResponse;
use sdv_vpm_vpm_system_service_rpc::Interface;
use std::sync::Arc;
use tokio::sync::mpsc::channel;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender as MpscSender;
use tokio::sync::watch::Receiver as WatchReceiver;

const VEHICLE_STATE_CHANNEL_QUEUE_SIZE: usize = 1;

#[automock]
#[async_trait]
// The trait encapsulates the functions of the service bundle.
// It also helps with testing the VPM agent while mocking the middleware generated code.
pub trait VpmService: Send + Sync {
    // Starts the rpc server that receives incoming RPC requests from the OEM to set the vehicle and power state in the SDV VM.
    async fn start(&mut self, vehicle_state_publisher: Option<Publisher<VehicleStateChange>>);
}

pub struct VpmServiceImpl {
    vehicle_state_receiver: Receiver<VpmVehicleState>,
}

impl VpmServiceImpl {
    // Returns the new vpm service and the RPC server used for handling the RPC calls.
    pub fn new(
        power_state_request_sender: MpscSender<PowerRequest>,
        power_state_watch_receiver: WatchReceiver<Arc<dyn PowerState>>,
    ) -> (Self, RpcServer) {
        let (vehicle_state_sender, vehicle_state_receiver) =
            channel(VEHICLE_STATE_CHANNEL_QUEUE_SIZE);
        let power_state_request_handler = Box::new(PowerStateRequestHandlerImpl::new(
            power_state_request_sender,
            power_state_watch_receiver,
        ));
        (
            Self { vehicle_state_receiver },
            RpcServer::new(vehicle_state_sender, power_state_request_handler),
        )
    }
}

#[async_trait]
impl VpmService for VpmServiceImpl {
    async fn start(&mut self, vehicle_state_publisher: Option<Publisher<VehicleStateChange>>) {
        if vehicle_state_publisher.is_none() {
            error!("Vehicle state publisher could not be started!");
            return;
        }
        let vehicle_state_publisher = vehicle_state_publisher.unwrap();

        while let Some(vehicle_state) = self.vehicle_state_receiver.recv().await {
            let mut vehicle_state_change_message = VehicleStateChange::new();
            vehicle_state_change_message.vehicle_state = vehicle_state.into();
            info!("Publishing Vpm vehicle mode, {:?}!", vehicle_state);
            let result = vehicle_state_publisher.publish(&vehicle_state_change_message);
            if result.is_err() {
                error!("Failed to publish vehicle state {:?}", vehicle_state);
            }
        }
    }
}

/// A listener struct that handles incoming RPC requests for setting the vpm.
pub struct RpcServer {
    vehicle_state_sender: MpscSender<VpmVehicleState>,
    power_state_request_handler: Box<dyn PowerStateRequestHandler>,
}

impl RpcServer {
    pub fn new(
        vehicle_state_sender: MpscSender<VpmVehicleState>,
        power_state_request_handler: Box<dyn PowerStateRequestHandler>,
    ) -> Self {
        Self { vehicle_state_sender, power_state_request_handler }
    }
}

/// Middleware RPC server trait implementation for RPC server.
#[allow(non_snake_case, missing_docs)]
#[async_trait::async_trait]
impl Interface for RpcServer {
    async fn SetVehicleState(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        request: &SetVehicleStateRequest,
    ) -> SdvResult<SetVehicleStateResponse> {
        info!("Received Vehicle state change request! {}", request);
        let state = request.vehicle_state.enum_value_or_default();
        if state == VpmVehicleState::VEHICLE_STATE_UNSPECIFIED {
            return Err(SdvStatus::with_message(
                SdvStatusCode::Internal,
                "Vehicle state was not set!".to_string(),
            ));
        }
        let result = self.vehicle_state_sender.send(state).await;
        if result.is_err() {
            return Err(SdvStatus::with_message(
                SdvStatusCode::Internal,
                "Could not publish vehicle state!".to_string(),
            ));
        }
        Ok(SetVehicleStateResponse::new())
    }

    async fn SetPowerState(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        request: &PowerStateChangeRequest,
    ) -> SdvResult<PowerStateChangeResponse> {
        let result = self.power_state_request_handler.handle_set_power_state(request.clone()).await;
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
    use crate::power_service::MockPowerStateRequestHandler;
    use crate::system_service::RpcServer;
    use crate::system_service::VEHICLE_STATE_CHANNEL_QUEUE_SIZE;
    use anyhow::anyhow;
    use sdv_comms::id::ServiceFqin;
    use sdv_status_rs::SdvStatusCode;
    use sdv_vpm_api::vehicle::VpmVehicleState;
    use sdv_vpm_api::vpm::power_state_change_response::PowerStateChangeRequestError;
    use sdv_vpm_api::vpm::PowerStateChangeRequest;
    use sdv_vpm_api::vpm::PowerStateRequest;
    use sdv_vpm_api::vpm::SetVehicleStateRequest;
    use sdv_vpm_api::vpm::ShutdownType;
    use sdv_vpm_vpm_system_service_rpc::Interface;
    use std::str::FromStr;
    use tokio::sync::mpsc::channel;

    #[tokio::test]
    async fn set_rpc_vehicle_state() {
        // Given: mock services.
        let (vehicle_state_sender, mut vehicle_state_receiver) =
            channel(VEHICLE_STATE_CHANNEL_QUEUE_SIZE);
        let mock_power_state_request_handler = MockPowerStateRequestHandler::default();
        let rpc_server =
            RpcServer::new(vehicle_state_sender, Box::new(mock_power_state_request_handler));
        let vehicle_state_handle = tokio::spawn(async move { vehicle_state_receiver.recv().await });

        // When: a vehicle state request is sent
        let request = SetVehicleStateRequest {
            vehicle_state: VpmVehicleState::VEHICLE_ON.into(),
            ..Default::default()
        };
        let fqin = ServiceFqin::from_str("vm:sdv.vpm.VpmSystemServiceBundle/default")
            .expect("Could not create context for tests!");
        let result = rpc_server.SetVehicleState(fqin, &request).await;

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
        let (vehicle_state_sender, _) = channel(VEHICLE_STATE_CHANNEL_QUEUE_SIZE);
        let mock_power_state_request_handler = MockPowerStateRequestHandler::default();
        let rpc_server =
            RpcServer::new(vehicle_state_sender, Box::new(mock_power_state_request_handler));

        // When: a vehicle state request is sent without proper state.
        let request = SetVehicleStateRequest { ..Default::default() };

        // Then: result should be error.
        let fqin = ServiceFqin::from_str("vm:sdv.vpm.VpmSystemServiceBundle/default")
            .expect("Could not create context for tests!");
        let err = rpc_server.SetVehicleState(fqin, &request).await.err().unwrap();
        assert_eq!(err.status_code(), SdvStatusCode::Internal);
    }

    #[tokio::test]
    async fn set_rpc_vehicle_state_fails_no_receiver() {
        // Given: mock services without a receiver.
        let (vehicle_state_sender, vehicle_state_receiver) =
            channel(VEHICLE_STATE_CHANNEL_QUEUE_SIZE);
        let mock_power_state_request_handler = MockPowerStateRequestHandler::default();
        let rpc_server =
            RpcServer::new(vehicle_state_sender, Box::new(mock_power_state_request_handler));

        // When: a vehicle state request is sent and receiver is dropped.
        let request = SetVehicleStateRequest {
            vehicle_state: VpmVehicleState::VEHICLE_ON.into(),
            ..Default::default()
        };
        drop(vehicle_state_receiver);

        // Then: result should be error.
        let fqin = ServiceFqin::from_str("vm:sdv.vpm.VpmSystemServiceBundle/default")
            .expect("Could not create context for tests!");
        let err = rpc_server.SetVehicleState(fqin, &request).await.err().unwrap();
        assert_eq!(err.status_code(), SdvStatusCode::Internal);
    }

    #[tokio::test]
    async fn set_rpc_power_state_succeeds() {
        // Given: a mock power service.
        let (vehicle_state_sender, _) = channel(VEHICLE_STATE_CHANNEL_QUEUE_SIZE);
        let mut mock_power_state_request_handler = MockPowerStateRequestHandler::default();
        mock_power_state_request_handler
            .expect_handle_set_power_state()
            .times(1)
            .returning(|_| Ok(()));
        let rpc_server =
            RpcServer::new(vehicle_state_sender, Box::new(mock_power_state_request_handler));

        // When: a power state request is sent
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::FINISH_SHUTDOWN.into(),
            shutdown_type: ShutdownType::POWER_OFF.into(),
            ..Default::default()
        };
        let fqin = ServiceFqin::from_str("vm:sdv.vpm.VpmSystemServiceBundle/default")
            .expect("Could not create context for tests!");
        let response = rpc_server.SetPowerState(fqin, &request).await;

        // Then: power state should be set in the power service.
        assert_eq!(response.unwrap().error, PowerStateChangeRequestError::NO_ERROR.into());
    }

    #[tokio::test]
    async fn set_rpc_power_state_fails() {
        // Given: a mock power service.
        let (vehicle_state_sender, _) = channel(VEHICLE_STATE_CHANNEL_QUEUE_SIZE);
        let mut mock_power_state_request_handler = MockPowerStateRequestHandler::default();
        mock_power_state_request_handler
            .expect_handle_set_power_state()
            .times(1)
            .returning(|_| Err(anyhow!("Power service failed")));
        let rpc_server =
            RpcServer::new(vehicle_state_sender, Box::new(mock_power_state_request_handler));

        // When: a power state request is sent but fails.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::FINISH_SHUTDOWN.into(),
            shutdown_type: ShutdownType::POWER_OFF.into(),
            ..Default::default()
        };
        let fqin = ServiceFqin::from_str("vm:sdv.vpm.VpmSystemServiceBundle/default")
            .expect("Could not create context for tests!");
        let response = rpc_server.SetPowerState(fqin, &request).await;

        // Then: power state should not be set in the power service.
        // Failure is reported.
        assert_eq!(
            response.unwrap().error,
            PowerStateChangeRequestError::INVALID_TRANSITION.into()
        );
    }
}
