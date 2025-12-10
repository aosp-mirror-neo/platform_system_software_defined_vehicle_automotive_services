// Copyright 2025 Google LLC

use async_trait::async_trait;
use log::warn;

use sdv_uds_comms::models::request::UdsRequest;
use sdv_uds_comms::models::response::{
    NegativeResponseCode, RequestedService, UdsResponse, UdsService,
};
use sdv_uds_comms::traits::UdsRequestHandler;
use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};

use crate::uds::data_item_service::DataItemRequestHandler;

/// Callback invoked when the server successfully binds to a socket address.
type ListeningCallback = Arc<dyn Fn(SocketAddr) + Send + Sync + 'static>;

/// Handles inbound UDS messages via appropriate UDS service implementation.
#[derive(Clone)]
pub struct UdsServer {
    data_item_service: Arc<OnceLock<Box<dyn DataItemRequestHandler>>>,
    on_listening: Option<ListeningCallback>,
}

/// Server errors when handling UDS messages.
#[derive(Debug, thiserror::Error)]
pub enum UdsServerError {}

impl UdsServer {
    /// Creates a new `UdsServer` without on_listening callback.
    pub fn new() -> Self {
        Self { data_item_service: Arc::new(OnceLock::new()), on_listening: None }
    }

    /// Attaches a callback to be executed once the server begins accepting connections.
    pub fn with_on_listening_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(SocketAddr) + Send + Sync + 'static,
    {
        self.on_listening = Some(Arc::new(callback));
        self
    }

    /// Enable communication from UDS server to Compliance Service Bundle via comms stack.
    pub fn set_data_item_service(&self, data_item_service: Box<dyn DataItemRequestHandler>) {
        if self.data_item_service.set(data_item_service).is_err() {
            warn!("Comms stack activation failed: already initialized");
        }
    }
}

#[async_trait]
impl UdsRequestHandler for UdsServer {
    type Error = UdsServerError;

    async fn handle_request(&self, req: UdsRequest) -> Result<UdsResponse, Self::Error> {
        let Some(data_item_service) = self.data_item_service.get() else {
            warn!("Waiting for comms stack, cannot yet process UDS request: {}", req.service);
            return Ok(UdsResponse::NegativeResponse {
                service: RequestedService::Known(req.service),
                code: NegativeResponseCode::BusyRepeatRequest,
            });
        };

        match req.service {
            UdsService::ReadDataByIdentifier => Ok(data_item_service.read_data(req).await),
            service => {
                warn!("Received unsupported UDS request: {}", service);
                Ok(UdsResponse::NegativeResponse {
                    service: RequestedService::Known(service),
                    code: NegativeResponseCode::ServiceNotSupported,
                })
            }
        }
    }

    async fn on_listening(&self, server_addr: SocketAddr) {
        if let Some(callback) = &self.on_listening {
            (callback)(server_addr);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdv_uds_comms::models::services::ReadDataByIdentifierResponseBuilder;

    struct MockDataItemService {
        response: UdsResponse,
    }

    #[async_trait]
    impl DataItemRequestHandler for MockDataItemService {
        async fn read_data(&self, _req: UdsRequest) -> UdsResponse {
            self.response.clone()
        }
    }

    #[tokio::test]
    async fn requests_fails_when_data_item_service_not_set() {
        let server = UdsServer::new();
        let req = UdsRequest { service: UdsService::ReadDataByIdentifier, payload: vec![] };

        let response = server.handle_request(req).await.unwrap();
        assert!(matches!(
            response,
            UdsResponse::NegativeResponse { code: NegativeResponseCode::BusyRepeatRequest, .. }
        ));
    }

    #[tokio::test]
    async fn read_data_request_succeeds() {
        let expected_response: UdsResponse =
            ReadDataByIdentifierResponseBuilder::new().build().into();
        let mock_service = MockDataItemService { response: expected_response.clone() };
        let server = UdsServer::new();
        server.set_data_item_service(Box::new(mock_service));

        let req = UdsRequest { service: UdsService::ReadDataByIdentifier, payload: vec![] };

        let response = server.handle_request(req).await.unwrap();
        assert_eq!(response, expected_response);
    }

    #[tokio::test]
    async fn unsupported_request_fails() {
        let unused_response = UdsResponse::PositiveResponse {
            service: UdsService::ReadDataByIdentifier,
            payload: vec![],
        };
        let mock_service = MockDataItemService { response: unused_response };
        let server = UdsServer::new();
        server.set_data_item_service(Box::new(mock_service));

        let req = UdsRequest { service: UdsService::DiagnosticSessionControl, payload: vec![] };

        let response = server.handle_request(req).await.unwrap();
        assert!(matches!(
            response,
            UdsResponse::NegativeResponse { code: NegativeResponseCode::ServiceNotSupported, .. }
        ));
    }
}
