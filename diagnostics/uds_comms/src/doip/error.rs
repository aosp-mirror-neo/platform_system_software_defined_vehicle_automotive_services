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
    /// A diagnostic message was received for a different logical address.
    #[error("Received DiagnosticMessage for wrong target address 0x{1:04X}, expected 0x{0:04X}")]
    WrongTargetAddress(u16, u16),
    /// Underlying I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
