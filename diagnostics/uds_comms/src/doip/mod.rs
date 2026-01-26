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
