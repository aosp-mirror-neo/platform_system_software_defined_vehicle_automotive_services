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

use crate::models::request::{UdsRequest, UdsService};
use crate::models::response::UdsResponse;

/// ReadDataByIdentifier service models.
pub mod read_data_by_identifier;
/// WriteDataByIdentifier service models.
pub mod write_data_by_identifier;

// -- Flatten

pub use read_data_by_identifier::*;
pub use write_data_by_identifier::*;

// -- Service model traits

/// Trait for typed UDS service requests.
pub trait UdsServiceRequest: Into<UdsRequest> {
    /// Returns the UdsService associated with this request.
    fn service(&self) -> UdsService;
}

/// Trait for typed UDS service responses.
pub trait UdsServiceResponse: Into<UdsResponse> {
    /// Returns the UdsService associated with this response.
    fn service(&self) -> UdsService;
}
