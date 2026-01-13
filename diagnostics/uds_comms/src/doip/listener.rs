/*
 * Copyright (C) 2026 The Android Open Source Project
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use super::codec::DoipCodec;
use super::error::DoipTransportError;
use super::logical_address::LogicalAddress;
use super::util::{build_message, DoipPayloadExt};
use crate::models::error::UdsCommsError;
use crate::models::request::UdsRequest;
use crate::models::response::{NegativeResponseCode, RequestedService, UdsResponse, UdsService};
use crate::traits::{Listener, UdsRequestHandler};
use async_trait::async_trait;
use doip_definitions::message::DoipMessage;
use doip_definitions::payload::{
    ActivationCode, AliveCheckResponse, DiagnosticAckCode, DiagnosticMessage, DiagnosticMessageAck,
    DiagnosticMessageNack, DiagnosticNackCode, DoipPayload, RoutingActivationRequest,
    RoutingActivationResponse,
};
use futures::{Future, SinkExt, StreamExt};
use log::{debug, error, info, warn};
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use tokio_util::codec::Framed;

/// Configuration for the `DoipListener`.
#[derive(Clone, Debug)]
pub struct DoipListenerConfig {
    /// The socket address to bind the TCP listener to.
    pub socket_addr: SocketAddr,
    /// The logical address of this ECU.
    pub logical_addr: LogicalAddress,
}

/// Manages inbound DoIP communication, implementing the `Listener` trait.
pub struct DoipListener<H>
where
    H: UdsRequestHandler + Send + Sync + 'static,
{
    config: DoipListenerConfig,
    handler: Arc<H>,
}

/// Manages the state and logic for a single client connection.
struct ConnectionHandler<H, T>
where
    H: UdsRequestHandler + Send + Sync + 'static,
    T: AsyncRead + AsyncWrite + Unpin,
{
    framed: Framed<T, DoipCodec>,
    client_addr: SocketAddr,
    config: DoipListenerConfig,
    handler: Arc<H>,
    client_logical_addr: Option<LogicalAddress>,
}

impl<H, T> ConnectionHandler<H, T>
where
    H: UdsRequestHandler + Send + Sync + 'static,
    T: AsyncRead + AsyncWrite + Unpin,
{
    /// Creates a new `ConnectionHandler`.
    fn new(
        transport: T,
        client_addr: SocketAddr,
        config: DoipListenerConfig,
        handler: Arc<H>,
    ) -> Self {
        Self {
            framed: Framed::new(transport, DoipCodec {}),
            client_addr,
            config,
            handler,
            client_logical_addr: None,
        }
    }

    /// Runs the connection handling loop, processing incoming messages
    /// until the client disconnects or a fatal transport error occurs.
    async fn run(&mut self) {
        loop {
            let doip_message = match self.framed.next().await {
                Some(Ok(msg)) => msg,
                Some(Err(e)) => {
                    error!("Connection error for client {}: {:?}", self.client_addr, e);
                    self.handler
                        .on_connection_failure(self.client_addr, UdsCommsError::from(e))
                        .await;
                    return;
                }
                None => {
                    info!("Client {} disconnected.", self.client_addr);
                    self.handler.on_disconnect(self.client_addr).await;
                    return;
                }
            };

            let payload_type = doip_message.payload.payload_type();

            debug!("Received {:?} message from {}", payload_type, self.client_addr);

            if let Err(e) = self.handle_message(doip_message).await {
                error!(
                    "Error processing {:?} message from {}: {}",
                    payload_type, self.client_addr, e
                );
            }
        }
    }

    /// Handles a single DoIP message, dispatching to the appropriate handler.
    async fn handle_message(&mut self, doip_message: DoipMessage) -> Result<(), UdsCommsError> {
        match doip_message.payload {
            DoipPayload::AliveCheckRequest(_) => self.handle_alive_check().await,
            DoipPayload::DiagnosticMessage(diag_msg) => {
                self.handle_diagnostic_message(diag_msg).await
            }
            DoipPayload::RoutingActivationRequest(request) => {
                self.handle_routing_activation(request).await
            }
            _ => {
                debug!("Ignoring message of type: {:?}", doip_message.payload.payload_type());
                Ok(())
            }
        }
    }

    /// Handles an alive check request.
    async fn handle_alive_check(&mut self) -> Result<(), UdsCommsError> {
        let doip_response = build_message(DoipPayload::AliveCheckResponse(AliveCheckResponse {
            source_address: *self.config.logical_addr.as_bytes(),
        }));

        debug!("Sending alive check response to {}", self.client_addr);
        self.framed.send(doip_response).await?;
        Ok(())
    }

    /// Handles a single diagnostic message.
    async fn handle_diagnostic_message(
        &mut self,
        diag_msg: DiagnosticMessage,
    ) -> Result<(), UdsCommsError> {
        // Use the client address from the request if no other one was provided
        // Note: we do not enforce routing activation to configure the client address
        // opposed to the ISO standrard to stay more flexibel for testing purposes.
        let client_addr: LogicalAddress = if let Some(client_addr) = self.client_logical_addr {
            client_addr
        } else {
            diag_msg.source_address.into()
        };

        // Check if DoIP message is addressed to us
        let target_addr: LogicalAddress = diag_msg.target_address.into();
        if target_addr != self.config.logical_addr {
            return self
                .send_diagnostic_nack(client_addr, DiagnosticNackCode::UnknownTargetAddress)
                .await;
        }

        // Confirm receipt of valid DoIP message
        self.send_diagnostic_ack(client_addr).await?;

        // Process UDS request in DoIP payload and send response back
        let uds_response = self.process_diagnostic_payload(&diag_msg.message).await;

        let doip_response = build_message(DoipPayload::DiagnosticMessage(DiagnosticMessage {
            source_address: *self.config.logical_addr.as_bytes(),
            target_address: *client_addr.as_bytes(),
            message: uds_response.into(),
        }));

        self.framed.send(doip_response).await?;
        Ok(())
    }

    /// Handles the initial routing activation phase.
    async fn handle_routing_activation(
        &mut self,
        request: RoutingActivationRequest,
    ) -> Result<(), UdsCommsError> {
        let client_logical_addr: LogicalAddress = request.source_address.into();

        let doip_response =
            build_message(DoipPayload::RoutingActivationResponse(RoutingActivationResponse {
                logical_address: request.source_address,
                source_address: *self.config.logical_addr.as_bytes(),
                activation_code: ActivationCode::SuccessfullyActivated,
                buffer: [0; 4],
            }));

        debug!("Sending routing activation response to {}", self.client_addr);
        self.framed.send(doip_response).await?;
        self.client_logical_addr = Some(client_logical_addr);
        Ok(())
    }

    /// Sends a single diagnostic acknowledge message.
    async fn send_diagnostic_ack(
        &mut self,
        target_address: LogicalAddress,
    ) -> Result<(), UdsCommsError> {
        let doip_response =
            build_message(DoipPayload::DiagnosticMessageAck(DiagnosticMessageAck {
                source_address: *self.config.logical_addr.as_bytes(),
                target_address: *target_address.as_bytes(),
                ack_code: DiagnosticAckCode::Acknowledged,
            }));

        debug!("Sending diagnostic ACK to {}", self.client_addr);
        self.framed.send(doip_response).await?;
        Ok(())
    }

    /// Sends a single diagnostic NOT acknowledge message.
    async fn send_diagnostic_nack(
        &mut self,
        target_address: LogicalAddress,
        nack_code: DiagnosticNackCode,
    ) -> Result<(), UdsCommsError> {
        let doip_response =
            build_message(DoipPayload::DiagnosticMessageNack(DiagnosticMessageNack {
                source_address: *self.config.logical_addr.as_bytes(),
                target_address: *target_address.as_bytes(),
                nack_code,
            }));

        debug!("Sending diagnostic NACK to {}", self.client_addr);
        self.framed.send(doip_response).await?;
        Ok(())
    }

    /// Processes a raw UDS payload: parses it, invokes the handler,
    /// and manages positive/negative responses.
    async fn process_diagnostic_payload(&self, payload: &[u8]) -> UdsResponse {
        let uds_request = match UdsRequest::try_from(payload) {
            Ok(request) => request,
            Err(e) => {
                warn!("UDS request parsing failed: {}", e);
                // Helper to decide if it's an Unknown Service or Invalid Format
                let (service, code) = match payload.first() {
                    Some(service_id_byte) => match UdsService::try_from(*service_id_byte) {
                        Ok(service) => (
                            RequestedService::Known(service),
                            NegativeResponseCode::IncorrectMessageLengthOrInvalidFormat,
                        ),
                        Err(_) => (
                            RequestedService::Unknown(*service_id_byte),
                            NegativeResponseCode::ServiceNotSupported,
                        ),
                    },
                    None => (
                        RequestedService::Unknown(0x00),
                        NegativeResponseCode::IncorrectMessageLengthOrInvalidFormat,
                    ),
                };
                return UdsResponse::NegativeResponse { service, code };
            }
        };

        let service_id = uds_request.service;

        match self.handler.handle_request(uds_request).await {
            Ok(response) => response,
            Err(e) => {
                warn!("UDS request handling failed: {}", e);
                UdsResponse::NegativeResponse {
                    service: RequestedService::Known(service_id),
                    code: NegativeResponseCode::GeneralReject,
                }
            }
        }
    }
}

#[async_trait]
impl<H> Listener<H> for DoipListener<H>
where
    H: UdsRequestHandler + Send + Sync + 'static,
{
    type Config = DoipListenerConfig;
    type Error = UdsCommsError;
    type StartFuture = Pin<Box<dyn Future<Output = Result<(), Self::Error>> + Send>>;

    fn new(config: Self::Config, handler: H) -> Self {
        Self { config, handler: Arc::new(handler) }
    }

    fn start(self) -> Self::StartFuture {
        Box::pin(async move {
            let listener = TcpListener::bind(self.config.socket_addr)
                .await
                .map_err(DoipTransportError::from)?;

            info!("Listening for connections on {}", self.config.socket_addr);
            self.handler.on_listening(self.config.socket_addr).await;

            loop {
                let (stream, client_addr) =
                    listener.accept().await.map_err(DoipTransportError::from)?;

                info!("Accepted client connection from {}", client_addr);
                self.handler.on_connection_accepted(self.config.socket_addr).await;

                let config = self.config.clone();
                let handler = self.handler.clone();

                tokio::spawn(async move {
                    ConnectionHandler::new(stream, client_addr, config, handler).run().await;
                });
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::response::UdsResponse;
    use doip_definitions::header::PayloadType;
    use doip_definitions::payload::{ActivationType, AliveCheckRequest};
    use std::sync::Mutex;
    use tokio::io::{duplex, DuplexStream};
    use tokio::net::TcpStream;
    use tokio_util::codec::Framed;

    const ECU_LOGICAL_ADDR: LogicalAddress = LogicalAddress::new(0x1000); // 4096
    const CLIENT_LOGICAL_ADDR: LogicalAddress = LogicalAddress::new(0x0E00);

    /// A mock `UdsRequestHandler` for testing purposes.
    #[derive(Clone)]
    struct MockUdsHandler {
        response: Arc<Mutex<Option<Result<UdsResponse, MockError>>>>,
        disconnect_called: Arc<Mutex<bool>>,
        connection_failure_called: Arc<Mutex<bool>>,
        listening_called: Arc<Mutex<bool>>,
        connection_accepted_called: Arc<Mutex<bool>>,
    }

    impl MockUdsHandler {
        fn new() -> Self {
            MockUdsHandler {
                response: Arc::new(Mutex::new(None)),
                disconnect_called: Arc::new(Mutex::new(false)),
                connection_failure_called: Arc::new(Mutex::new(false)),
                listening_called: Arc::new(Mutex::new(false)),
                connection_accepted_called: Arc::new(Mutex::new(false)),
            }
        }

        fn set_response(&self, response: Result<UdsResponse, MockError>) {
            *self.response.lock().unwrap() = Some(response);
        }
    }

    #[derive(Debug, thiserror::Error)]
    #[error("Mock error occurred: {message}.")]
    struct MockError {
        message: String,
    }

    #[async_trait]
    impl UdsRequestHandler for MockUdsHandler {
        type Error = MockError;
        async fn handle_request(&self, request: UdsRequest) -> Result<UdsResponse, MockError> {
            if let Some(response) = self.response.lock().unwrap().take() {
                return response;
            }
            // Default echo implementation for testing
            Ok(UdsResponse::PositiveResponse { service: request.service, payload: request.payload })
        }

        async fn on_disconnect(&self, _: SocketAddr) {
            *self.disconnect_called.lock().unwrap() = true
        }

        async fn on_connection_failure(&self, _: SocketAddr, _: UdsCommsError) {
            *self.connection_failure_called.lock().unwrap() = true
        }

        async fn on_listening(&self, _socket_addr: SocketAddr) {
            *self.listening_called.lock().unwrap() = true
        }

        async fn on_connection_accepted(&self, _client_addr: SocketAddr) {
            *self.connection_accepted_called.lock().unwrap() = true
        }
    }

    fn create_test_config(addr: SocketAddr) -> DoipListenerConfig {
        DoipListenerConfig { socket_addr: addr, logical_addr: ECU_LOGICAL_ADDR }
    }

    fn create_server_connection(
        stream: DuplexStream,
    ) -> (ConnectionHandler<MockUdsHandler, DuplexStream>, Arc<MockUdsHandler>) {
        let addr = "127.0.0.1:0".parse().unwrap();
        let config = create_test_config(addr);
        let handler = Arc::new(MockUdsHandler::new());

        let mut conn = ConnectionHandler::new(stream, addr, config, Arc::clone(&handler));
        // Pre-activate routing for most tests
        conn.client_logical_addr = Some(CLIENT_LOGICAL_ADDR);
        (conn, handler)
    }

    async fn create_listener() -> (DoipListener<MockUdsHandler>, SocketAddr, MockUdsHandler) {
        // Bind to a random available port
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener); // Drop listener so the DoipListener can bind to it.

        let config = create_test_config(addr);
        let handler = MockUdsHandler::new();
        let doip_listener = DoipListener::new(config, handler.clone());

        (doip_listener, addr, handler)
    }

    #[tokio::test]
    async fn test_handle_routing_activation() {
        // Simulate client sending a routing activation request
        let request = RoutingActivationRequest {
            source_address: *CLIENT_LOGICAL_ADDR.as_bytes(),
            activation_type: ActivationType::Default,
            buffer: [0; 4],
        };
        let doip_request = build_message(DoipPayload::RoutingActivationRequest(request));

        let (client_stream, server_stream) = duplex(4096);
        let (mut server_connection, _) = create_server_connection(server_stream);
        server_connection.client_logical_addr = None; // Not yet activated

        let client_logic = async {
            let mut framed_client = Framed::new(client_stream, DoipCodec {});
            let response = framed_client.next().await.unwrap().unwrap();
            assert_eq!(response.payload.payload_type(), PayloadType::RoutingActivationResponse);

            if let DoipPayload::RoutingActivationResponse(payload) = response.payload {
                assert_eq!(payload.logical_address, *CLIENT_LOGICAL_ADDR.as_bytes());
                assert_eq!(payload.source_address, *ECU_LOGICAL_ADDR.as_bytes());
                assert_eq!(payload.activation_code, ActivationCode::SuccessfullyActivated);
            } else {
                panic!("Expected RoutingActivationResponse");
            }
        };

        let server_logic = server_connection.handle_message(doip_request);
        let _handle = tokio::join!(client_logic, server_logic);
    }

    #[tokio::test]
    async fn test_handle_alive_check() {
        let alive_check = DoipPayload::AliveCheckRequest(AliveCheckRequest {});
        let doip_request = build_message(alive_check);

        let (client_stream, server_stream) = duplex(4096);
        let (mut server_connection, _) = create_server_connection(server_stream);

        let client_logic = async {
            let mut framed_client = Framed::new(client_stream, DoipCodec {});
            let response = framed_client.next().await.unwrap().unwrap();
            assert_eq!(response.payload.payload_type(), PayloadType::AliveCheckResponse);

            if let DoipPayload::AliveCheckResponse(payload) = response.payload {
                assert_eq!(payload.source_address, *ECU_LOGICAL_ADDR.as_bytes());
            } else {
                panic!("Expected AliveCheckResponse");
            }
        };

        let server_logic = server_connection.handle_message(doip_request);
        let _handle = tokio::join!(client_logic, server_logic);
    }

    #[tokio::test]
    async fn test_handle_diagnostic_message_wrong_target() {
        let (mut client_stream, server_stream) = duplex(4096);
        let (mut server_connection, _) = create_server_connection(server_stream);

        let wrong_target_addr = LogicalAddress::new(0);
        let doip_request = build_message(DoipPayload::DiagnosticMessage(DiagnosticMessage {
            source_address: *CLIENT_LOGICAL_ADDR.as_bytes(), // Use a different source address
            target_address: *wrong_target_addr.as_bytes(),
            message: vec![0x10, 0x01],
        }));

        let result = server_connection.handle_message(doip_request).await;

        // Check that a NACK was sent
        let mut framed_client = Framed::new(&mut client_stream, DoipCodec {});
        let nack_response = framed_client.next().await.unwrap().unwrap();
        assert_eq!(nack_response.payload.payload_type(), PayloadType::DiagnosticMessageNack);

        if let DoipPayload::DiagnosticMessageNack(payload) = nack_response.payload {
            assert_eq!(payload.source_address, *ECU_LOGICAL_ADDR.as_bytes());
            assert_eq!(payload.target_address, *CLIENT_LOGICAL_ADDR.as_bytes());
            assert_eq!(payload.nack_code, DiagnosticNackCode::UnknownTargetAddress);
        } else {
            panic!("Expected DiagnosticMessageNack");
        }

        // The handler should return Ok(()) after successfully sending a NACK.
        assert!(result.is_ok());
    }

    /// Helper to verify processing of UDS responses in diagnostic messages
    async fn verify_diagnostic_uds_message_processing(
        request_payload: Vec<u8>,
        expected_response: Vec<u8>,
        handler_response: Option<UdsResponse>,
    ) {
        let diag_message = DoipPayload::DiagnosticMessage(DiagnosticMessage {
            source_address: *CLIENT_LOGICAL_ADDR.as_bytes(),
            target_address: *ECU_LOGICAL_ADDR.as_bytes(),
            message: request_payload,
        });
        let doip_request = build_message(diag_message);

        let (client_stream, server_stream) = duplex(4096);
        let (mut server_connection, _) = create_server_connection(server_stream);

        if let Some(handler_response) = handler_response {
            server_connection.handler.set_response(Ok(handler_response));
        }

        let client_logic = async {
            let mut framed_client = Framed::new(client_stream, DoipCodec {});
            // 1. Expect a Diagnostic ACK first, since the DoIP message itself is valid.
            let ack_response = framed_client.next().await.unwrap().unwrap();
            assert_eq!(ack_response.payload.payload_type(), PayloadType::DiagnosticMessageAck);

            // 2. Expect a Diagnostic Message containing the UDS Response.
            let response = framed_client.next().await.unwrap().unwrap();
            assert_eq!(response.payload.payload_type(), PayloadType::DiagnosticMessage);

            if let DoipPayload::DiagnosticMessage(payload) = response.payload {
                assert_eq!(payload.message, expected_response);
            } else {
                panic!("Expected DiagnosticMessage containing a UDS Response");
            }
        };

        let server_logic = server_connection.handle_message(doip_request);
        let (_, server_result) = tokio::join!(client_logic, server_logic);
        // The handler should return Ok(()) after successfully sending the response.
        assert!(server_result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_diagnostic_message_success() {
        verify_diagnostic_uds_message_processing(
            // Valid UDS request: ReadDataByIdentifier DID F190
            vec![0x22, 0xF1, 0x90],
            // The message should be a UDS Positive Response:
            // 0x62 (Positive response SID is original SID offset by 0x40)
            // 0xF1 (echo of request payload)
            // 0x90 (echo of request payload)
            vec![0x62, 0xF1, 0x90],
            None,
        )
        .await;
    }

    #[tokio::test]
    async fn test_handle_diagnostic_message_with_handler_response() {
        verify_diagnostic_uds_message_processing(
            // Valid UDS request: ReadDataByIdentifier DID F190
            vec![0x22, 0xF1, 0x90],
            // The message should be the raw negative UDS response from below:
            // 0x7F (Negative Response SID)
            // 0x22 (ReadDataByIdentifier SID)
            // 0x10 (GeneralReject)
            vec![0x7F, 0x22, 0x10],
            Some(UdsResponse::NegativeResponse {
                service: RequestedService::Known(UdsService::ReadDataByIdentifier),
                code: NegativeResponseCode::GeneralReject,
            }),
        )
        .await;
    }

    #[tokio::test]
    async fn test_handle_diagnostic_message_malformed_uds_payload() {
        verify_diagnostic_uds_message_processing(
            // Empty UDS payload is malformed
            vec![],
            // The message should be a UDS Negative Response:
            // 0x7F (Negative Response SID)
            // 0x00 (Unknown default SID)
            // 0x13 (IncorrectMessageLengthOrInvalidFormat)
            vec![0x7F, 0x00, 0x13],
            None,
        )
        .await;
    }

    #[tokio::test]
    async fn test_handle_diagnostic_message_unknown_service() {
        verify_diagnostic_uds_message_processing(
            // Empty UDS payload is malformed
            vec![0x99],
            // The message should be a UDS Negative Response:
            // 0x7F (Negative Response SID)
            // 0x99 (Unknown request SID)
            // 0x11 (ServiceNotSupported)
            vec![0x7F, 0x99, 0x11],
            None,
        )
        .await;
    }

    #[tokio::test]
    async fn test_full_listener_flow() {
        let (doip_listener, addr, _) = create_listener().await;

        // Start the listener in the background
        tokio::spawn(doip_listener.start());
        // Give it a moment to start up
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // --- Client simulation ---
        let stream = TcpStream::connect(addr).await.unwrap();
        let mut framed_client = Framed::new(stream, DoipCodec {});

        // 1. Send Routing Activation
        let routing_req =
            build_message(DoipPayload::RoutingActivationRequest(RoutingActivationRequest {
                source_address: *CLIENT_LOGICAL_ADDR.as_bytes(),
                activation_type: ActivationType::Default,
                buffer: [0; 4],
            }));
        framed_client.send(routing_req).await.unwrap();
        let routing_resp = framed_client.next().await.unwrap().unwrap();
        assert_eq!(routing_resp.payload.payload_type(), PayloadType::RoutingActivationResponse);

        // 2. Send Diagnostic Message
        let diag_req = build_message(DoipPayload::DiagnosticMessage(DiagnosticMessage {
            source_address: *CLIENT_LOGICAL_ADDR.as_bytes(),
            target_address: *ECU_LOGICAL_ADDR.as_bytes(),
            message: vec![0x22, 0xF1, 0x90],
        }));
        framed_client.send(diag_req).await.unwrap();

        // Expect ACK first
        let diag_ack = framed_client.next().await.unwrap().unwrap();
        assert_eq!(diag_ack.payload.payload_type(), PayloadType::DiagnosticMessageAck);

        let diag_resp = framed_client.next().await.unwrap().unwrap();
        assert_eq!(diag_resp.payload.payload_type(), PayloadType::DiagnosticMessage);
        if let DoipPayload::DiagnosticMessage(p) = diag_resp.payload {
            assert_eq!(p.message, vec![0x62, 0xF1, 0x90]);
        }
    }

    #[tokio::test]
    async fn test_on_disconnect_is_called() {
        let (client_stream, server_stream) = duplex(4096);
        let (mut server_connection, handler) = create_server_connection(server_stream);

        // Drop the client stream to simulate a disconnect
        drop(client_stream);

        // The run loop should exit after detecting the disconnect
        server_connection.run().await;

        assert!(*handler.disconnect_called.lock().unwrap());
        assert!(!*handler.connection_failure_called.lock().unwrap());
    }

    #[tokio::test]
    async fn test_on_connection_failure_is_called() {
        let (mut client_stream, server_stream) = duplex(4096);
        let (mut server_connection, handler) = create_server_connection(server_stream);

        // Write invalid data to cause a decoding error
        use tokio::io::AsyncWriteExt;
        client_stream.write_all(&[0xDE, 0xAD, 0xBE, 0xEF]).await.unwrap();
        drop(client_stream);

        server_connection.run().await;

        assert!(*handler.connection_failure_called.lock().unwrap());
        assert!(!*handler.disconnect_called.lock().unwrap());
    }

    #[tokio::test]
    async fn test_on_listening_is_called() {
        let (doip_listener, _, handler) = create_listener().await;
        let listener_handle = tokio::spawn(doip_listener.start());
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        assert!(*handler.listening_called.lock().unwrap(), "on_listening should be called");
        listener_handle.abort();
    }

    #[tokio::test]
    async fn test_on_connection_accepted_is_called() {
        let (doip_listener, addr, handler) = create_listener().await;

        // Start the listener in the background and wait for the callbacks to be triggered
        let listener_handle = tokio::spawn(doip_listener.start());
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Connect a client to trigger on_connection_accepted
        let _stream = TcpStream::connect(addr).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        assert!(
            *handler.connection_accepted_called.lock().unwrap(),
            "on_connection_accepted should be called"
        );
        listener_handle.abort();
    }
}
