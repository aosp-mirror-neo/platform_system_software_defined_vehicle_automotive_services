// Copyright 2025 Google LLC

/// DoIP codec to use frames on top of a byte sequence.
pub mod codec;

/// Connector for outbound DoIP connections.
pub mod connector;

/// Listener for inbound DoIP connections.
pub mod listener;

/// DoIP specific error types.
pub mod error;

/// DoIP utility functions and traits.
pub mod util;

/// Abstraction for logical address type.
pub mod logical_address;
