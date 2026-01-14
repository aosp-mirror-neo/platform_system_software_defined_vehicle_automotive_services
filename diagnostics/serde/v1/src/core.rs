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
//! UDS serde library
//!
//! Implements serialization/deserialization between UDS and protobuf formats

/// Protobuf <-> UDS translator
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

/// Utilities for serialization/deserialization of three-byte integers.
mod integer_utils;

/// Calculation of size of message representation in diagnostics format.
mod message_size;

#[cfg(test)]
pub mod runtime_message_descriptor;
