// Copyright 2024 Google LLC
//! UDS serde library
//!
//! Implements serialization/deserialization between UDS and protobuf formats

/// Public translation APIs
pub mod diagnostics_translator;

/// Defines ids for diagnostic entities to be translated.
pub mod entity_id;

#[cfg(test)]
pub mod runtime_message_descriptor;

/// Expose public API from the core library.
pub use sdv_uds_serde_v1_core::translator;
