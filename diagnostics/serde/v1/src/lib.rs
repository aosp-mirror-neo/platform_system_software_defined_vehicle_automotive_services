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

/// Public translation APIs
pub mod diagnostics_translator;

/// Defines ids for diagnostic entities to be translated.
pub mod entity_id;

#[cfg(test)]
pub mod runtime_message_descriptor;

/// Expose public API from the core library.
pub use sdv_uds_serde_v1_core::translator;
