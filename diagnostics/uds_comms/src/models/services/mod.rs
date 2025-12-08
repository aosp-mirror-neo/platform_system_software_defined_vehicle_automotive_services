// Copyright 2025 Google LLC

use crate::models::request::{UdsRequest, UdsService};
use crate::models::response::UdsResponse;

/// ReadDataByIdentifier service models.
pub mod read_data_by_identifier;

// -- Flatten

pub use read_data_by_identifier::*;

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
