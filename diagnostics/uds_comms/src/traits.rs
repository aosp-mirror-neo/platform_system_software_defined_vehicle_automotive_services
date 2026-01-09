// Copyright 2025 Google LLC

use crate::models::error::UdsCommsError;
use crate::models::request::UdsRequest;
use crate::models::response::UdsResponse;
use async_trait::async_trait;
use std::error::Error;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use tower::Service;

/// A boxed future that can be sent across threads.
pub type BoxFuture<T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'static>>;

/// An abstraction over a transport mechanism for establishing and managing
/// an outbound channel for sending and receiving message payloads.
#[async_trait]
pub trait Connector:
    Service<
        UdsRequest,
        Response = UdsResponse,
        Error = UdsCommsError,
        Future = BoxFuture<UdsResponse, UdsCommsError>,
    > + Send
{
    /// Asynchronously establishes a connection (e.g., performs DoIP routing activation).
    async fn connect(&mut self) -> Result<(), UdsCommsError>;

    /// Asynchronously terminates the connection.
    async fn disconnect(&mut self) -> Result<(), UdsCommsError>;
}

/// Abstraction for a service that listens for and accepts inbound transport connections.
/// It is generic over the handler (H) that processes the extracted byte stream.
pub trait Listener<H>: Sized + Send + 'static
where
    H: UdsRequestHandler + Send + Sync + 'static,
{
    /// The configuration required to create a new listener.
    type Config;

    /// The error type that can be returned by the listener.
    type Error: Error + Send + 'static;

    /// The future returned by the `start` method.
    type StartFuture: Future<Output = Result<(), Self::Error>> + Send;

    /// Creates a new instance of the listener.
    fn new(config: Self::Config, handler: H) -> Self;

    /// Starts the transport listener (e.g., TCP binding on 13400) and begins delegation.
    fn start(self) -> Self::StartFuture;
}

/// A trait that defines the contract for handling UDS requests.
#[async_trait]
pub trait UdsRequestHandler: Send + Sync {
    /// The error type that can be returned when handling a request.
    type Error: Error + Send + Sync + 'static;

    /// Handles an incoming request and returns a response.
    ///
    /// # Arguments
    /// * `request` - A slice containing the raw request as bytes.
    ///
    /// # Returns
    /// A `Result` containing the raw response as bytes or an error.
    async fn handle_request(&self, request: UdsRequest) -> Result<UdsResponse, Self::Error>;

    // --- Lifecycle Callbacks ---

    /// Callback triggered when the TCP listener is bound and ready to accept connections.
    async fn on_listening(&self, _socket_addr: SocketAddr) {
        // Default implementation: do nothing
    }

    /// Callback triggered immediately when a new TCP connection is accepted,
    /// before the connection handler loop spawns.
    async fn on_connection_accepted(&self, _client_addr: SocketAddr) {
        // Default implementation: do nothing
    }

    /// Callback for the handler when a client disconnects (socket closed cleanly).
    async fn on_disconnect(&self, _client_addr: SocketAddr) {
        // Default implementation: do nothing
    }

    /// Callback for the handler when a client connection fails (socket error).
    async fn on_connection_failure(&self, _client_addr: SocketAddr, _error: UdsCommsError) {
        // Default implementation: do nothing
    }
}
