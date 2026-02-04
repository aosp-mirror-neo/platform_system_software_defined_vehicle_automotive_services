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

use async_trait::async_trait;
use sdv_uds_comms::doip::connector::{DoipConnector, DoipConnectorConfig};
use sdv_uds_comms::doip::listener::{DoipListener, DoipListenerConfig};
use sdv_uds_comms::doip::logical_address::LogicalAddress;
use sdv_uds_comms::models::request::{UdsRequest, UdsService};
use sdv_uds_comms::models::response::UdsResponse;
use sdv_uds_comms::traits::{Connector, Listener, UdsRequestHandler};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::sync::Notify;
use tower::Service;

const CLIENT_LOGICAL_ADDR: u16 = 0x0E00;
const ECU_LOGICAL_ADDR: u16 = 0x1007;

/// A mock `UdsRequestHandler` for testing purposes. It echoes back valid requests.
#[derive(Clone, Default)]
struct MockHandler {
    last_request: Arc<Mutex<Option<UdsRequest>>>,
    ready: Arc<Notify>,
}

#[async_trait]
impl UdsRequestHandler for MockHandler {
    type Error = std::io::Error;

    // Record the last request and echo back a positive response
    async fn handle_request(&self, request: UdsRequest) -> Result<UdsResponse, Self::Error> {
        *self.last_request.lock().unwrap() = Some(request.clone());
        Ok(UdsResponse::PositiveResponse { service: request.service, payload: request.payload })
    }

    async fn on_listening(&self, _socket_addr: SocketAddr) {
        self.ready.notify_waiters();
    }
}

/// Helper to create a `DoipListener` with a `MockHandler`.
async fn setup_listener() -> (DoipListener<MockHandler>, SocketAddr, MockHandler) {
    // find a port we can safely bind against to avoid EADDRINUSE errors
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    let config = DoipListenerConfig {
        socket_addr: addr,
        logical_addr: LogicalAddress::new(ECU_LOGICAL_ADDR),
    };
    let handler = MockHandler::default();
    let doip_listener = DoipListener::new(config, handler.clone());
    (doip_listener, addr, handler)
}

/// Helper function to set up a DoIP listener and connector for end-to-end tests.
async fn setup_doip_e2e_test(
    ecu_logical_addr_config: Option<u16>,
) -> (
    DoipConnector,
    MockHandler,
    tokio::task::JoinHandle<Result<(), sdv_uds_comms::models::error::UdsCommsError>>,
) {
    let (listener, addr, handler) = setup_listener().await;
    let listener_handle = tokio::spawn(listener.start());
    handler.ready.notified().await; // Wait for listener to be ready

    let mut connector = DoipConnector::new(DoipConnectorConfig {
        ecu_socket_addr: Some(addr),
        ecu_logical_addr: ecu_logical_addr_config,
        source_logical_addr: CLIENT_LOGICAL_ADDR,
    });

    connector.connect().await.expect("Connector connection failed during setup");

    (connector, handler, listener_handle)
}

/// Helper function to tear down a DoIP connector and listener after an end-to-end test.
async fn teardown_doip_e2e_test(
    mut connector: DoipConnector,
    listener_handle: tokio::task::JoinHandle<
        Result<(), sdv_uds_comms::models::error::UdsCommsError>,
    >,
) {
    connector.disconnect().await.expect("Connector disconnect failed during teardown");
    listener_handle.abort();
}

/// Helper function to run a DoIP end-to-end test with setup and teardown.
async fn run_doip_e2e_test<F>(ecu_logical_addr: Option<u16>, test_body: F)
where
    F: for<'a> AsyncFn(&'a mut DoipConnector, MockHandler),
{
    let (mut connector, handler, listener_handle) = setup_doip_e2e_test(ecu_logical_addr).await;
    test_body(&mut connector, handler).await;
    teardown_doip_e2e_test(connector, listener_handle).await;
}

#[tokio::test]
async fn test_e2e_with_preconfigured_address() {
    run_doip_e2e_test(Some(ECU_LOGICAL_ADDR), async |connector, handler| {
        let request =
            UdsRequest { service: UdsService::ReadDataByIdentifier, payload: vec![0xF1, 0x90] };
        let response = connector.call(request.clone()).await.expect("Call failed");

        let last_req =
            handler.last_request.lock().unwrap().take().expect("Handler did not receive a request");
        assert_eq!(last_req, request);

        let expected_response =
            UdsResponse::PositiveResponse { service: request.service, payload: request.payload };
        assert_eq!(response, expected_response);
    })
    .await;
}

#[tokio::test]
async fn test_e2e_with_routing_activation() {
    run_doip_e2e_test(None, async |connector, handler| {
        let request =
            UdsRequest { service: UdsService::ReadDataByIdentifier, payload: vec![0xF1, 0x88] };
        let response = connector.call(request.clone()).await.expect("Call failed");

        let last_req =
            handler.last_request.lock().unwrap().take().expect("Handler did not receive a request");
        assert_eq!(last_req, request);

        let expected_response =
            UdsResponse::PositiveResponse { service: request.service, payload: request.payload };
        assert_eq!(response, expected_response);
    })
    .await;
}

/// Helper function to assert that a UDS service is supported.
async fn assert_uds_service_support(
    connector: &mut DoipConnector,
    handler: &MockHandler,
    request: UdsRequest,
    service_name: &str,
) {
    let response = connector
        .call(request.clone())
        .await
        .unwrap_or_else(|e| panic!("Call failed for {}: {:?}", service_name, e));

    let last_req = handler
        .last_request
        .lock()
        .unwrap()
        .take()
        .unwrap_or_else(|| panic!("Handler did not receive a {} request", service_name));
    assert_eq!(last_req, request);

    let expected_response =
        UdsResponse::PositiveResponse { service: request.service, payload: request.payload };
    assert_eq!(response, expected_response);
}

#[tokio::test]
async fn test_e2e_supported_uds_services() {
    run_doip_e2e_test(Some(ECU_LOGICAL_ADDR), async |connector, handler| {
        let read_request =
            UdsRequest { service: UdsService::ReadDataByIdentifier, payload: vec![0xF1, 0x90] };
        assert_uds_service_support(connector, &handler, read_request, "ReadDataByIdentifier").await;

        let write_request = UdsRequest {
            service: UdsService::WriteDataByIdentifier,
            payload: vec![0xF1, 0x90, 0xDE, 0xAD, 0xBE, 0xEF],
        };
        assert_uds_service_support(connector, &handler, write_request, "WriteDataByIdentifier")
            .await;
    })
    .await;
}
