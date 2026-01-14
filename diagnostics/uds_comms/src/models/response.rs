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

pub use automotive_diag::uds::{UdsCommand as UdsService, UdsError as NegativeResponseCode};
use std::convert::TryFrom;

const NEGATIVE_RESPONSE_SID: u8 = 0x7F;
const POSITIVE_RESPONSE_SID_OFFSET: u8 = 0x40;

/// Wrapper to handle supported and unsupported service identifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestedService {
    /// A known, valid service defined in the UdsService enum.
    Known(UdsService),
    /// An unknown or unsupported service identifier (raw byte).
    Unknown(u8),
}

/// Represents a generic UDS response, which can be positive or negative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UdsResponse {
    /// A positive response (success)
    PositiveResponse {
        /// The UDS service that was executed
        service: UdsService,
        /// The service-specific response payload
        payload: Vec<u8>,
    },
    /// A negative response (failure)
    NegativeResponse {
        /// The UDS service that failed
        service: RequestedService,
        /// The Negative Response Code (NRC)
        code: NegativeResponseCode,
    },
}

impl UdsResponse {
    /// UDS service identifier (SID) of the original request.
    pub fn request_sid(&self) -> u8 {
        match self {
            UdsResponse::PositiveResponse { service, .. } => *service as u8,
            UdsResponse::NegativeResponse { service, .. } => match service {
                RequestedService::Known(service) => *service as u8,
                RequestedService::Unknown(sid) => *sid,
            },
        }
    }

    /// UDS service identifier (SID) of this response.
    ///
    /// A negative response SID is always 0x7F.
    /// A positive response SID is the request SID + 0x40.
    pub fn response_sid(&self) -> u8 {
        match self {
            UdsResponse::PositiveResponse { .. } => {
                self.request_sid() + POSITIVE_RESPONSE_SID_OFFSET
            }
            UdsResponse::NegativeResponse { .. } => NEGATIVE_RESPONSE_SID,
        }
    }
}

/// UDS response translation errors
#[derive(Debug, PartialEq, thiserror::Error)]
pub enum UdsResponseTranslationError {
    /// The byte slice is empty.
    #[error("Received no bytes")]
    ReceivedNoBytes,
    /// The byte slice has an incorrect length.
    #[error("Incorrect message length")]
    IncorrectMessageLength,
    /// The Negative Response Code (NRC) is not supported.
    #[error("Unsupported Negative Response Code (NRC): {0}")]
    UnsupportedNegativeResponseCode(u8),
    /// The request Service Identifier (SID) is not supported.
    #[error("Unsupported request Service Identifier (SID): {0}")]
    UnsupportedRequestServiceId(u8),
    /// The response Service Identifier (SID) is not supported.
    #[error("Unsupported response Service Identifier (SID): {0}")]
    UnsupportedResponseServiceId(u8),
}

impl TryFrom<&[u8]> for UdsResponse {
    type Error = UdsResponseTranslationError;

    /// Parses a raw UDS response byte slice into a typed UdsResponse.
    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.is_empty() {
            return Err(UdsResponseTranslationError::ReceivedNoBytes);
        }

        let response_sid_byte = bytes[0];

        // Check for Negative Response
        if response_sid_byte == NEGATIVE_RESPONSE_SID {
            if bytes.len() < 3 {
                return Err(UdsResponseTranslationError::IncorrectMessageLength);
            }
            let request_sid_byte = bytes[1];

            let service = match UdsService::try_from(request_sid_byte) {
                Ok(known_service) => RequestedService::Known(known_service),
                Err(_) => RequestedService::Unknown(request_sid_byte),
            };

            let nrc_byte = bytes[2];
            let code = NegativeResponseCode::try_from(nrc_byte).map_err(|_| {
                UdsResponseTranslationError::UnsupportedNegativeResponseCode(nrc_byte)
            })?;

            return Ok(UdsResponse::NegativeResponse { service, code });
        }

        // A Positive Response SID has a fixed offset from the request SID
        if response_sid_byte < POSITIVE_RESPONSE_SID_OFFSET {
            return Err(UdsResponseTranslationError::UnsupportedResponseServiceId(
                response_sid_byte,
            ));
        }

        // Handle Positive Response
        let request_sid_byte = response_sid_byte - POSITIVE_RESPONSE_SID_OFFSET;
        let service = UdsService::try_from(request_sid_byte).map_err(|_| {
            UdsResponseTranslationError::UnsupportedRequestServiceId(request_sid_byte)
        })?;

        let payload = bytes[1..].to_vec();
        Ok(UdsResponse::PositiveResponse { service, payload })
    }
}

impl From<UdsResponse> for Vec<u8> {
    /// Serializes a typed UdsResponse into a byte vector.
    fn from(response: UdsResponse) -> Self {
        let mut bytes = vec![response.response_sid()];

        let payload = match response {
            UdsResponse::PositiveResponse { payload, .. } => payload,
            UdsResponse::NegativeResponse { code, .. } => {
                vec![response.request_sid(), code as u8]
            }
        };

        bytes.extend(payload);
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_positive_response_serialization_deserialization() {
        let response = UdsResponse::PositiveResponse {
            service: UdsService::ReadDataByIdentifier, // SID 0x22
            payload: vec![0xF1, 0x90, 0xAA, 0xBB],     // DIDs 0xF190, 0xAABB
        };
        let bytes: Vec<u8> = response.clone().into();
        // Response SID for 0x22 is 0x62
        assert_eq!(bytes, vec![0x62, 0xF1, 0x90, 0xAA, 0xBB]);

        let parsed_response = UdsResponse::try_from(bytes.as_slice()).unwrap();
        assert_eq!(response, parsed_response);
    }

    #[test]
    fn test_negative_response_serialization_deserialization() {
        let response = UdsResponse::NegativeResponse {
            service: RequestedService::Known(UdsService::DiagnosticSessionControl),
            code: NegativeResponseCode::SubFunctionNotSupported,
        };
        let bytes: Vec<u8> = response.clone().into();
        // Negative response format: 0x7F <Request SID> <NRC>
        assert_eq!(bytes, vec![0x7F, 0x10, 0x12]);

        let parsed_response = UdsResponse::try_from(bytes.as_slice()).unwrap();
        assert_eq!(response, parsed_response);
    }

    #[test]
    fn test_negative_response_unknown_sid_serialization_deserialization() {
        let response = UdsResponse::NegativeResponse {
            service: RequestedService::Unknown(0x00),
            code: NegativeResponseCode::ServiceNotSupported,
        };
        let bytes: Vec<u8> = response.clone().into();
        // Negative response format: 0x7F <Request SID> <NRC>
        assert_eq!(bytes, vec![0x7F, 0x00, 0x11]);

        let parsed_response = UdsResponse::try_from(bytes.as_slice()).unwrap();
        assert_eq!(response, parsed_response);
    }

    #[test]
    fn test_response_deserialization_empty() {
        let bytes = vec![];
        let result = UdsResponse::try_from(bytes.as_slice());
        assert_eq!(result, Err(UdsResponseTranslationError::ReceivedNoBytes));
    }

    #[test]
    fn test_negative_response_deserialization_too_short() {
        let bytes = vec![0x7F, 0x10]; // Missing NRC
        let result = UdsResponse::try_from(bytes.as_slice());
        assert_eq!(result, Err(UdsResponseTranslationError::IncorrectMessageLength));
    }

    #[test]
    fn test_negative_response_deserialization_unsupported_nrc() {
        let bytes = vec![0x7F, 0x10, 0xFF]; // unsupported NRC
        let result = UdsResponse::try_from(bytes.as_slice());
        assert_eq!(result, Err(UdsResponseTranslationError::UnsupportedNegativeResponseCode(0xFF)));
    }

    #[test]
    fn test_positive_response_deserialization_unsupported_sid() {
        let bytes = vec![0x01, 0x02]; // unsupported response SID (0x01 - 0x40 would be negative)
        let result = UdsResponse::try_from(bytes.as_slice());
        assert_eq!(result, Err(UdsResponseTranslationError::UnsupportedResponseServiceId(0x01)));
    }

    #[test]
    fn test_negative_response_sid_methods() {
        let neg_response = UdsResponse::NegativeResponse {
            service: RequestedService::Known(UdsService::ReadDataByIdentifier),
            code: NegativeResponseCode::RequestOutOfRange,
        };
        assert_eq!(neg_response.request_sid(), 0x22);
        assert_eq!(neg_response.response_sid(), 0x7F);
    }

    #[test]
    fn test_positive_response_sid_methods() {
        let pos_response = UdsResponse::PositiveResponse {
            service: UdsService::ReadDataByIdentifier,
            payload: vec![],
        };
        assert_eq!(pos_response.request_sid(), 0x22);
        assert_eq!(pos_response.response_sid(), 0x62); // 0x22 + 0x40
    }
}
