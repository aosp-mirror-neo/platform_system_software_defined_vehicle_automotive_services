// Copyright 2025 Google LLC

use super::codec::DoipCodec;
use super::error::DoipTransportError;
use super::util::build_message;
use crate::models::error::UdsCommsError;
use crate::models::request::UdsRequest;
use crate::models::response::UdsResponse;
use crate::traits::{BoxFuture, Connector};

use async_trait::async_trait;
use doip_definitions::message::DoipMessage;
use doip_definitions::payload::{
    ActivationCode, ActivationType, AliveCheckResponse, DiagnosticMessage, DoipPayload,
    RoutingActivationRequest,
};
use futures::{SinkExt, StreamExt};
use log::info;
use std::net::SocketAddr;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};
use tokio_util::codec::Framed;
use tower::Service;

const TIMEOUT: Duration = Duration::from_secs(5);

/// Configuration for the DoipConnector.
#[derive(Clone)]
pub struct DoipConnectorConfig {
    /// The socket address of the target ECU with IP and port.
    ///
    /// If None, UDP broadcast should be performed - not implemented yet.
    pub ecu_socket_addr: Option<SocketAddr>,
    /// The logical address of the target ECU.
    ///
    /// If None, routing activation will be performed.
    pub ecu_logical_addr: Option<u16>,
    /// The logical address of the client sending the request.
    pub source_logical_addr: u16,
}

/// Represents an active DoIP connection.
#[derive(Clone)]
struct ActiveConnection {
    /// Framed TcpStream connection, avoiding concurrent read/write operations.
    framed: Arc<Mutex<Framed<TcpStream, DoipCodec>>>,
    /// The logical address of the target ECU.
    ecu_logical_addr: u16,
}

/// Manages outbound DoIP communication, implementing the `Connector` with `tower::Service` trait.
pub struct DoipConnector {
    /// Configuration for the DoipConnector.
    config: DoipConnectorConfig,
    /// The active connection, if one exists.
    connection: Option<ActiveConnection>,
}

impl DoipConnector {
    /// Creates a new `DoipConnector`.
    pub fn new(config: DoipConnectorConfig) -> Self {
        DoipConnector { config, connection: None }
    }

    async fn perform_routing_activation(
        &self,
        framed: &Arc<Mutex<Framed<TcpStream, DoipCodec>>>,
    ) -> Result<u16, UdsCommsError> {
        let request_message =
            build_message(DoipPayload::RoutingActivationRequest(RoutingActivationRequest {
                source_address: self.config.source_logical_addr.to_be_bytes(),
                activation_type: ActivationType::Default,
                buffer: [0; 4], // Per ISO, this is reserved and set to 0
            }));

        let framed_arc = Arc::clone(framed);
        let mut locked_framed = framed_arc.lock().await;

        info!("Sending RoutingActivationRequest");
        locked_framed.send(request_message).await?;

        // Wait for RoutingActivationResponse
        let response_message = timeout(TIMEOUT, locked_framed.next())
            .await
            .map_err(|_| DoipTransportError::TimedOut("Routing Activation".to_string()))?
            .ok_or(DoipTransportError::ConnectionAborted)?
            .map_err(UdsCommsError::from)?;

        match response_message.payload {
            DoipPayload::RoutingActivationResponse(resp) => {
                if resp.activation_code == ActivationCode::SuccessfullyActivated {
                    info!("Routing successfully activated for ECU {:?}", resp.source_address);
                    Ok(u16::from_be_bytes(resp.source_address))
                } else {
                    Err(DoipTransportError::RoutingActivationFailed(resp.activation_code))?
                }
            }
            other => Err(DoipTransportError::UnexpectedMessage(DoipMessage {
                header: response_message.header,
                payload: other,
            }))?,
        }
    }

    /// Creates a DoIP Alive Check Response message with the given source address.
    fn create_alive_check_response(source_address: u16) -> DoipMessage {
        build_message(DoipPayload::AliveCheckResponse(AliveCheckResponse {
            source_address: source_address.to_be_bytes(),
        }))
    }
}

/// Implementation of the `Connector` trait.
#[async_trait]
impl Connector for DoipConnector {
    /// Establishes the DoIP connection if not active already.
    async fn connect(&mut self) -> Result<(), UdsCommsError> {
        if self.connection.is_some() {
            return Err(DoipTransportError::AlreadyConnected)?;
        }

        let socket_addr = self
            .config
            .ecu_socket_addr
            // TODO: Implement UDP discovery when target not configured b/437284420
            .ok_or(DoipTransportError::EcuTargetNotConfigured)?;

        // Establish the TCP stream with a timeout.
        let stream = timeout(TIMEOUT, TcpStream::connect(socket_addr))
            .await
            .map_err(|_| DoipTransportError::TimedOut("TCP connection".to_string()))?
            .map_err(DoipTransportError::Io)?;

        info!("TCP Connected to ECU socket address {:?}", socket_addr);

        // Wrap the stream with the DoIP codec
        let framed = Framed::new(stream, DoipCodec {});
        let framed_arc = Arc::new(Mutex::new(framed));

        let ecu_logical_addr = if let Some(logical_addr) = self.config.ecu_logical_addr {
            logical_addr
        } else {
            self.perform_routing_activation(&framed_arc).await?
        };

        self.connection = Some(ActiveConnection { framed: framed_arc, ecu_logical_addr });
        Ok(())
    }

    /// Asynchronously terminates the connection.
    async fn disconnect(&mut self) -> Result<(), UdsCommsError> {
        let Some(conn) = self.connection.take() else {
            return Err(DoipTransportError::NotConnected)?;
        };
        let mut locked_framed = conn.framed.lock().await;
        info!("Disconnecting");

        // Get the inner TcpStream and shut it down
        locked_framed.get_mut().shutdown().await.map_err(DoipTransportError::Io)?;
        Ok(())
    }
}

/// Implementation of the required `tower::Service` trait.
impl Service<UdsRequest> for DoipConnector {
    type Response = UdsResponse;
    type Error = UdsCommsError;
    type Future = BoxFuture<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.connection.is_none() {
            return Poll::Ready(Err(DoipTransportError::NotConnected)?);
        }
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: UdsRequest) -> Self::Future {
        let Some(active_conn) = self.connection.as_ref().cloned() else {
            return Box::pin(async { Err(DoipTransportError::NotConnected)? });
        };

        let source_address = self.config.source_logical_addr;

        Box::pin(async move {
            let request_message =
                build_message(DoipPayload::DiagnosticMessage(DiagnosticMessage {
                    source_address: source_address.to_be_bytes(),
                    target_address: active_conn.ecu_logical_addr.to_be_bytes(),
                    message: request.clone().into(),
                }));
            let mut locked_framed = active_conn.framed.lock().await;

            info!("Sending UdsRequest {0} over DoIP", request.service);
            locked_framed.send(request_message).await?;

            loop {
                let response_message = timeout(TIMEOUT, locked_framed.next())
                    .await
                    .map_err(|_| {
                        DoipTransportError::TimedOut(format!(
                            "UDS response for {}",
                            request.service
                        ))
                    })?
                    .ok_or(DoipTransportError::ConnectionAborted)?
                    .map_err(UdsCommsError::from)?;

                match response_message.payload {
                    DoipPayload::DiagnosticMessage(DiagnosticMessage { message, .. }) => {
                        return UdsResponse::try_from(message.as_slice())
                            .map_err(UdsCommsError::from);
                    }
                    DoipPayload::DiagnosticMessageNack(nack) => {
                        return Err(DoipTransportError::DiagnosticNack(nack))?;
                    }
                    DoipPayload::AliveCheckRequest(_) => {
                        // Respond and continue waiting for diagnostic message
                        let response = DoipConnector::create_alive_check_response(source_address);
                        locked_framed.send(response).await?;
                    }
                    _ => continue, // Ignore valid but irrelevant messages
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doip::codec::DoipCodecError;
    use crate::doip::util::build_message;
    use crate::models::request::UdsService;
    use doip_definitions::payload::{
        ActivationCode, AliveCheckRequest, DiagnosticMessageNack, DiagnosticNackCode,
        EntityStatusRequest, RoutingActivationResponse,
    };
    use std::io;
    use tokio::net::TcpListener;
    use tower::ServiceExt;

    const CLIENT_LOGICAL_ADDR: u16 = 0x0E00;
    const ECU_LOGICAL_ADDR: u16 = 0x1007;

    enum RoutingActivationBehavior {
        Skip,
        Succeed,
        Fail,
    }

    enum DiagnosticMessageBehavior {
        Skip,
        UdsResponse,
        NotAcknowledged,
        AliveCheckThenUdsResponse,
        AliveCheckThenConnectionDrop,
    }

    /// Helper to start a mock TCP server, handle connections and perform assertions.
    async fn spawn_mock_ecu_server(
        routing_activation: RoutingActivationBehavior,
        diagnostic_message: DiagnosticMessageBehavior,
    ) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut framed = Framed::new(stream, DoipCodec {});

            match routing_activation {
                RoutingActivationBehavior::Succeed => {
                    handle_routing_activation(&mut framed, true).await.unwrap();
                }
                RoutingActivationBehavior::Fail => {
                    handle_routing_activation(&mut framed, false).await.unwrap();
                    return; // Stop if routing activation failed, as client will disconnect.
                }
                RoutingActivationBehavior::Skip => {}
            }

            if !matches!(diagnostic_message, DiagnosticMessageBehavior::Skip) {
                handle_diagnostic_message(&mut framed, diagnostic_message).await.unwrap();
            }

            // Server waits for client to disconnect
            let _ = framed.next().await;
        });

        addr
    }

    /// Handles the routing activation phase for the mock ECU server.
    async fn handle_routing_activation(
        framed: &mut Framed<TcpStream, DoipCodec>,
        positive_response: bool,
    ) -> Result<(), DoipCodecError> {
        let request_message = framed.next().await.ok_or(io::Error::new(
            io::ErrorKind::ConnectionAborted,
            "Stream ended unexpectedly",
        ))??;

        if let DoipPayload::RoutingActivationRequest(req) = request_message.payload {
            assert_eq!(u16::from_be_bytes(req.source_address), CLIENT_LOGICAL_ADDR);

            let response_payload = if positive_response {
                DoipPayload::RoutingActivationResponse(RoutingActivationResponse {
                    logical_address: req.source_address,
                    source_address: ECU_LOGICAL_ADDR.to_be_bytes(),
                    activation_code: ActivationCode::SuccessfullyActivated,
                    buffer: [0; 4],
                })
            } else {
                DoipPayload::RoutingActivationResponse(RoutingActivationResponse {
                    logical_address: [0; 2],
                    source_address: ECU_LOGICAL_ADDR.to_be_bytes(),
                    activation_code: ActivationCode::DeniedMissingAuthentication,
                    buffer: [0; 4],
                })
            };
            let response = build_message(response_payload);
            framed.send(response).await?;
        } else {
            Err(io::Error::new(io::ErrorKind::InvalidData, "Expected RoutingActivationRequest"))?
        }
        Ok(())
    }

    /// Handles the diagnostic message phase for the mock ECU server.
    async fn handle_diagnostic_message(
        framed: &mut Framed<TcpStream, DoipCodec>,
        diagnostic_message: DiagnosticMessageBehavior,
    ) -> Result<(), DoipCodecError> {
        let request_message = framed.next().await.ok_or(io::Error::new(
            io::ErrorKind::ConnectionAborted,
            "Stream ended unexpectedly",
        ))??;

        let DoipPayload::DiagnosticMessage(req) = request_message.payload else {
            panic!("Expected DiagnosticMessage");
        };

        assert_eq!(u16::from_be_bytes(req.source_address), CLIENT_LOGICAL_ADDR);
        assert_eq!(u16::from_be_bytes(req.target_address), ECU_LOGICAL_ADDR);
        assert_eq!(req.message, vec![0x22, 0xF1, 0x90]); // raw UdsRequest

        if matches!(diagnostic_message, DiagnosticMessageBehavior::AliveCheckThenUdsResponse) {
            verify_alive_check_handling(framed, false).await.unwrap();
        } else if matches!(
            diagnostic_message,
            DiagnosticMessageBehavior::AliveCheckThenConnectionDrop
        ) {
            // Send alive check and drop connection.
            verify_alive_check_handling(framed, true).await.unwrap();
            return Ok(());
        } else {
            // Send an irrelevant message first to test the client's loop
            framed
                .send(build_message(DoipPayload::EntityStatusRequest(EntityStatusRequest {})))
                .await?;
        }

        let response_payload = match diagnostic_message {
            DiagnosticMessageBehavior::UdsResponse
            | DiagnosticMessageBehavior::AliveCheckThenUdsResponse => {
                DoipPayload::DiagnosticMessage(DiagnosticMessage {
                    source_address: ECU_LOGICAL_ADDR.to_be_bytes(),
                    target_address: CLIENT_LOGICAL_ADDR.to_be_bytes(),
                    message: vec![0x62, 0xF1, 0x90, 0xDE, 0xAD, 0xBE, 0xEF], // raw UdsResponse
                })
            }
            DiagnosticMessageBehavior::NotAcknowledged => {
                DoipPayload::DiagnosticMessageNack(DiagnosticMessageNack {
                    source_address: ECU_LOGICAL_ADDR.to_be_bytes(),
                    target_address: CLIENT_LOGICAL_ADDR.to_be_bytes(),
                    nack_code: DiagnosticNackCode::InvalidSourceAddress,
                })
            }
            DiagnosticMessageBehavior::Skip
            | DiagnosticMessageBehavior::AliveCheckThenConnectionDrop => {
                return Ok(());
            }
        };

        framed.send(build_message(response_payload)).await?;
        Ok(())
    }

    /// Sends alive check for the mock ECU server.
    async fn verify_alive_check_handling(
        framed: &mut Framed<TcpStream, DoipCodec>,
        drop_connection: bool,
    ) -> Result<(), DoipCodecError> {
        // Send an alive check first
        let alive_check_req = build_message(DoipPayload::AliveCheckRequest(AliveCheckRequest {}));
        framed.send(alive_check_req).await?;

        // If the test expects a connection drop, we close the stream from the server side.
        if drop_connection && framed.get_mut().shutdown().await.is_err() {
            // Ignore error during shutdown, as the client might have already closed it.
            return Ok(());
        }

        // Wait for the alive check response from the client
        let alive_check_resp = framed.next().await.ok_or(io::Error::new(
            io::ErrorKind::ConnectionAborted,
            "Stream ended unexpectedly while waiting for AliveCheckResponse",
        ))??;

        if let DoipPayload::AliveCheckResponse(resp) = alive_check_resp.payload {
            assert_eq!(u16::from_be_bytes(resp.source_address), CLIENT_LOGICAL_ADDR);
        } else {
            panic!("Expected AliveCheckResponse");
        }
        Ok(())
    }

    #[tokio::test]
    async fn connect_fails_if_no_ecu_socket_addr() {
        let mut connector = DoipConnector::new(DoipConnectorConfig {
            ecu_socket_addr: None,
            ecu_logical_addr: None,
            source_logical_addr: CLIENT_LOGICAL_ADDR,
        });
        let result = connector.connect().await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            UdsCommsError::DoipTransport(DoipTransportError::EcuTargetNotConfigured)
        ));
    }

    #[tokio::test]
    async fn connect_succeeds_with_routing_activation() {
        let server_addr = spawn_mock_ecu_server(
            RoutingActivationBehavior::Succeed,
            DiagnosticMessageBehavior::Skip,
        )
        .await;
        let mut connector = DoipConnector::new(DoipConnectorConfig {
            ecu_socket_addr: Some(server_addr),
            ecu_logical_addr: None,
            source_logical_addr: CLIENT_LOGICAL_ADDR,
        });
        let result = connector.connect().await;
        assert!(result.is_ok());

        let Some(active_conn) = connector.connection.take() else {
            panic!("Expected connection to be set");
        };
        assert_eq!(active_conn.ecu_logical_addr, ECU_LOGICAL_ADDR);
    }

    #[tokio::test]
    async fn connect_fails_with_routing_activation() {
        let server_addr =
            spawn_mock_ecu_server(RoutingActivationBehavior::Fail, DiagnosticMessageBehavior::Skip)
                .await;
        let mut connector = DoipConnector::new(DoipConnectorConfig {
            ecu_socket_addr: Some(server_addr),
            ecu_logical_addr: None,
            source_logical_addr: CLIENT_LOGICAL_ADDR,
        });
        let result = connector.connect().await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            UdsCommsError::DoipTransport(DoipTransportError::RoutingActivationFailed(
                ActivationCode::DeniedMissingAuthentication
            ))
        ));
    }

    #[tokio::test]
    async fn connect_succeeds_with_preconfigured_logical_addr() {
        let server_addr =
            spawn_mock_ecu_server(RoutingActivationBehavior::Skip, DiagnosticMessageBehavior::Skip)
                .await; // Server won't do routing activation
        let mut connector = DoipConnector::new(DoipConnectorConfig {
            ecu_socket_addr: Some(server_addr),
            ecu_logical_addr: Some(ECU_LOGICAL_ADDR + 1),
            source_logical_addr: CLIENT_LOGICAL_ADDR,
        });
        let result = connector.connect().await;
        assert!(result.is_ok());

        let Some(active_conn) = connector.connection.take() else {
            panic!("Expected connection to be set");
        };
        assert_eq!(active_conn.ecu_logical_addr, ECU_LOGICAL_ADDR + 1);
    }

    #[tokio::test]
    async fn connect_fails_when_already_connected() {
        let server_addr =
            spawn_mock_ecu_server(RoutingActivationBehavior::Skip, DiagnosticMessageBehavior::Skip)
                .await; // Server won't do routing activation
        let mut connector = DoipConnector::new(DoipConnectorConfig {
            ecu_socket_addr: Some(server_addr),
            ecu_logical_addr: Some(ECU_LOGICAL_ADDR + 1),
            source_logical_addr: CLIENT_LOGICAL_ADDR,
        });
        let success_result = connector.connect().await;
        assert!(success_result.is_ok());

        let fail_result = connector.connect().await;
        assert!(fail_result.is_err());
        let err = fail_result.unwrap_err();
        assert!(matches!(err, UdsCommsError::DoipTransport(DoipTransportError::AlreadyConnected)));
    }

    #[tokio::test]
    async fn disconnect_fails_when_not_connected() {
        let mut connector = DoipConnector::new(DoipConnectorConfig {
            ecu_socket_addr: None,
            ecu_logical_addr: None,
            source_logical_addr: CLIENT_LOGICAL_ADDR,
        });
        let result = connector.disconnect().await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, UdsCommsError::DoipTransport(DoipTransportError::NotConnected)));
    }

    #[tokio::test]
    async fn disconnect_succeeds_when_connected() {
        let server_addr =
            spawn_mock_ecu_server(RoutingActivationBehavior::Skip, DiagnosticMessageBehavior::Skip)
                .await;
        let mut connector = DoipConnector::new(DoipConnectorConfig {
            ecu_socket_addr: Some(server_addr),
            ecu_logical_addr: Some(ECU_LOGICAL_ADDR),
            source_logical_addr: CLIENT_LOGICAL_ADDR,
        });
        connector.connect().await.unwrap();
        assert!(connector.connection.is_some());

        let result = connector.disconnect().await;
        assert!(result.is_ok());
        assert!(connector.connection.is_none());
    }

    #[tokio::test]
    async fn poll_ready_is_err_when_not_connected() {
        let mut connector = DoipConnector::new(DoipConnectorConfig {
            ecu_socket_addr: None,
            ecu_logical_addr: None,
            source_logical_addr: CLIENT_LOGICAL_ADDR,
        });
        let poll_result = futures::poll!(connector.ready());
        match poll_result {
            Poll::Ready(Err(UdsCommsError::DoipTransport(DoipTransportError::NotConnected))) => (),
            _ => panic!("Expected Poll::Ready(Err)"),
        }
    }

    #[tokio::test]
    async fn call_sends_and_receives_data() {
        let server_addr = spawn_mock_ecu_server(
            RoutingActivationBehavior::Skip,
            DiagnosticMessageBehavior::UdsResponse,
        )
        .await;
        let mut connector = DoipConnector::new(DoipConnectorConfig {
            ecu_socket_addr: Some(server_addr),
            ecu_logical_addr: Some(ECU_LOGICAL_ADDR),
            source_logical_addr: CLIENT_LOGICAL_ADDR,
        });
        connector.connect().await.unwrap();

        let request =
            UdsRequest { service: UdsService::ReadDataByIdentifier, payload: vec![0xF1, 0x90] };

        let result = connector.call(request).await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(
            response,
            UdsResponse::PositiveResponse {
                service: UdsService::ReadDataByIdentifier,
                payload: vec![0xF1, 0x90, 0xDE, 0xAD, 0xBE, 0xEF]
            }
        );
    }

    #[tokio::test]
    async fn call_handles_diagnostic_nack() {
        let server_addr = spawn_mock_ecu_server(
            RoutingActivationBehavior::Skip,
            DiagnosticMessageBehavior::NotAcknowledged,
        )
        .await;
        let mut connector = DoipConnector::new(DoipConnectorConfig {
            ecu_socket_addr: Some(server_addr),
            ecu_logical_addr: Some(ECU_LOGICAL_ADDR),
            source_logical_addr: CLIENT_LOGICAL_ADDR,
        });
        connector.connect().await.unwrap();

        let request =
            UdsRequest { service: UdsService::ReadDataByIdentifier, payload: vec![0xF1, 0x90] };

        let result = connector.call(request).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, UdsCommsError::DoipTransport(DoipTransportError::DiagnosticNack(_))));
    }

    #[tokio::test]
    async fn call_handles_alive_check_request_and_uds_response() {
        let server_addr = spawn_mock_ecu_server(
            RoutingActivationBehavior::Skip,
            DiagnosticMessageBehavior::AliveCheckThenUdsResponse,
        )
        .await;
        let mut connector = DoipConnector::new(DoipConnectorConfig {
            ecu_socket_addr: Some(server_addr),
            ecu_logical_addr: Some(ECU_LOGICAL_ADDR),
            source_logical_addr: CLIENT_LOGICAL_ADDR,
        });
        connector.connect().await.unwrap();

        let request =
            UdsRequest { service: UdsService::ReadDataByIdentifier, payload: vec![0xF1, 0x90] };

        let result = connector.call(request).await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(
            response,
            UdsResponse::PositiveResponse {
                service: UdsService::ReadDataByIdentifier,
                payload: vec![0xF1, 0x90, 0xDE, 0xAD, 0xBE, 0xEF]
            }
        );
    }

    #[tokio::test]
    async fn call_fails_on_connection_drop_after_alive_check_request() {
        let server_addr = spawn_mock_ecu_server(
            RoutingActivationBehavior::Skip,
            DiagnosticMessageBehavior::AliveCheckThenConnectionDrop,
        )
        .await;
        let mut connector = DoipConnector::new(DoipConnectorConfig {
            ecu_socket_addr: Some(server_addr),
            ecu_logical_addr: Some(ECU_LOGICAL_ADDR),
            source_logical_addr: CLIENT_LOGICAL_ADDR,
        });
        connector.connect().await.unwrap();

        let request =
            UdsRequest { service: UdsService::ReadDataByIdentifier, payload: vec![0xF1, 0x90] };

        let result = connector.call(request).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, UdsCommsError::DoipTransport(DoipTransportError::ConnectionAborted)),
            "Expected ConnectionAborted error, but got: {:?}",
            err
        );
    }

    #[tokio::test]
    async fn full_lifecycle_connect_call_disconnect() {
        let server_addr = spawn_mock_ecu_server(
            RoutingActivationBehavior::Succeed,
            DiagnosticMessageBehavior::UdsResponse,
        )
        .await;
        let mut connector = DoipConnector::new(DoipConnectorConfig {
            ecu_socket_addr: Some(server_addr),
            ecu_logical_addr: None,
            source_logical_addr: CLIENT_LOGICAL_ADDR,
        });

        // Connect
        assert!(connector.connect().await.is_ok());
        assert!(connector.connection.is_some());

        // Check readiness
        assert!(connector.ready().await.is_ok());

        // Make a call
        let request =
            UdsRequest { service: UdsService::ReadDataByIdentifier, payload: vec![0xF1, 0x90] };
        let response = connector.call(request).await.unwrap();
        assert_eq!(
            response,
            UdsResponse::PositiveResponse {
                service: UdsService::ReadDataByIdentifier,
                payload: vec![0xF1, 0x90, 0xDE, 0xAD, 0xBE, 0xEF]
            }
        );

        // Disconnect
        assert!(connector.disconnect().await.is_ok());
        assert!(connector.connection.is_none());

        // Check it's no longer ready
        let poll_result = futures::poll!(connector.ready());
        assert!(matches!(poll_result, Poll::Ready(Err(_))));
    }
}
