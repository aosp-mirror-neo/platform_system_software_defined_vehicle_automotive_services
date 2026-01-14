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

pub use automotive_diag::uds::UdsCommand as UdsService;
use std::convert::TryFrom;

/// Represents a generic UDS request with a specific service and payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UdsRequest {
    /// The UDS service to be executed
    pub service: UdsService,
    /// Service-specific payload (may include subfunction, DIDs, etc.)
    pub payload: Vec<u8>,
}

impl UdsRequest {
    /// UDS service identifier (SID) of the request.
    pub fn request_sid(&self) -> u8 {
        self.service as u8
    }
}

/// UDS request translation errors
#[derive(Debug, PartialEq, thiserror::Error)]
pub enum UdsRequestTranslationError {
    /// The byte slice is empty.
    #[error("Received no bytes")]
    ReceivedNoBytes,
    /// The Service Identifier (SID) is not supported.
    #[error("Unsupported Service Identifier (SID): {0}")]
    UnsupportedServiceId(u8),
}

impl TryFrom<&[u8]> for UdsRequest {
    type Error = UdsRequestTranslationError;

    /// Parses a raw UDS request byte slice into a typed UdsRequest.
    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.is_empty() {
            return Err(UdsRequestTranslationError::ReceivedNoBytes);
        }

        let sid_byte = bytes[0];
        let service = UdsService::try_from(sid_byte)
            .map_err(|_| UdsRequestTranslationError::UnsupportedServiceId(sid_byte))?;
        let payload = bytes[1..].to_vec();

        Ok(UdsRequest { service, payload })
    }
}

impl From<UdsRequest> for Vec<u8> {
    /// Serializes a typed UdsRequest into a byte vector.
    fn from(request: UdsRequest) -> Self {
        let mut bytes = vec![request.request_sid()];
        bytes.extend(request.payload);
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialization_deserialization() {
        let request = UdsRequest {
            service: UdsService::ReadDataByIdentifier,
            payload: vec![0xF1, 0x90], // DID F190
        };
        let bytes: Vec<u8> = request.clone().into();
        assert_eq!(bytes, vec![0x22, 0xF1, 0x90]);

        let parsed_request = UdsRequest::try_from(bytes.as_slice()).unwrap();
        assert_eq!(request, parsed_request);
    }

    #[test]
    fn test_deserialization_unsupported_sid() {
        let bytes = vec![0xFF, 0x01]; // unsupported SID
        let result = UdsRequest::try_from(bytes.as_slice());
        assert_eq!(result, Err(UdsRequestTranslationError::UnsupportedServiceId(0xFF)));
    }

    #[test]
    fn test_deserialization_no_bytes() {
        let bytes = vec![];
        let result = UdsRequest::try_from(bytes.as_slice());
        assert_eq!(result, Err(UdsRequestTranslationError::ReceivedNoBytes));
    }

    #[test]
    fn test_sid_methods() {
        let request = UdsRequest { service: UdsService::ReadDataByIdentifier, payload: vec![] };
        assert_eq!(request.request_sid(), 0x22);
    }
}
