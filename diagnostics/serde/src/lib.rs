// Copyright 2024 Google LLC
//! UDS serde library
//!
//! Implements serialization/deserialization between UDS and protobuf formats

/// Public translation APIs
pub mod translator;

/// serde::Deserialize implementation for protobuf message
mod protobuf_deserialize;
/// serde::Serialize implementation for protobuf message
mod protobuf_serialize;

/// serde::Deserializer implementation for UDS format
mod uds_deserializer;
/// serde::Serializer implementation for UDS format
mod uds_serializer;

/// Utilities for retrieving proto field's options that affect
/// serialization/deserialization.
mod field_options;

/// Utilities for retrieving proto enum's options that affect
/// serialization/deserialization.
mod enum_options;
