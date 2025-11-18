// Copyright 2025 Google LLC

use doip_definitions::message::DoipMessage;
use doip_definitions::payload::{ActivationCode, DiagnosticMessageNack};

/// Transport errors when sending and receiving DoIP messages.
#[derive(Debug, thiserror::Error)]
pub enum DoipTransportError {
    /// There is already an active DoIP connection.
    #[error("DoIP connection already exists.")]
    AlreadyConnected,
    /// There is no configured ECU address to connect to.
    #[error("ECU target address is not configured.")]
    EcuTargetNotConfigured,
    /// There is no DoIP connection.
    #[error("Missing DoIP connection.")]
    NotConnected,
    /// Routing activation failed.
    #[error("DoIP routing activation failed: {0:?}")]
    RoutingActivationFailed(ActivationCode),
    /// Connection aborted.
    #[error("DoIP connection was aborted.")]
    ConnectionAborted,
    /// The operation’s timeout expired, causing it to be canceled.
    #[error("{0} timed out.")]
    TimedOut(String),
    /// Received an unexpected DoIP message type.
    #[error("Received unexpected DoIP message: {0:?}")]
    UnexpectedMessage(DoipMessage),
    /// Received a negative acknowledgement for a diagnostic message.
    #[error("Received DiagnosticMessageNack: {0:?}")]
    DiagnosticNack(DiagnosticMessageNack),
    /// Underlying I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
