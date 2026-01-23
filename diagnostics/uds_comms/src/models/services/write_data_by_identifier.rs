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

use super::{UdsServiceRequest, UdsServiceResponse};
use crate::models::request::{UdsRequest, UdsService};
use crate::models::response::UdsResponse;

/// WriteDataByIdentifier request models.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteDataByIdentifierRequest {
    /// Data identifier (DID) to be written.
    pub data_id: u16,
    /// The raw data payload to write.
    pub data: Vec<u8>,
}

/// WriteDataByIdentifier response models.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteDataByIdentifierResponse {
    /// Data identifier (DID) that was written.
    pub data_id: u16,
}

// -- Request

/// WriteDataByIdentifier translation errors
#[derive(Debug, PartialEq, thiserror::Error)]
pub enum WriteDataByIdentifierTranslationError {
    /// Payload too short.
    #[error("Payload too short for WriteDataByIdentifier: expected at least 2 bytes.")]
    PayloadTooShort,
}

impl UdsServiceRequest for WriteDataByIdentifierRequest {
    fn service(&self) -> UdsService {
        UdsService::WriteDataByIdentifier
    }
}

impl From<WriteDataByIdentifierRequest> for UdsRequest {
    fn from(req: WriteDataByIdentifierRequest) -> Self {
        let service = req.service();
        let mut bytes = Vec::new();

        bytes.extend_from_slice(&req.data_id.to_be_bytes());
        bytes.extend(req.data);

        UdsRequest { service, payload: bytes }
    }
}

impl TryFrom<Vec<u8>> for WriteDataByIdentifierRequest {
    type Error = WriteDataByIdentifierTranslationError;

    fn try_from(payload: Vec<u8>) -> Result<Self, Self::Error> {
        if payload.len() < 2 {
            return Err(Self::Error::PayloadTooShort);
        }

        let data_id = u16::from_be_bytes([payload[0], payload[1]]);
        let data = payload[2..].to_vec();

        Ok(WriteDataByIdentifierRequest { data_id, data })
    }
}

// -- Response

impl UdsServiceResponse for WriteDataByIdentifierResponse {
    fn service(&self) -> UdsService {
        UdsService::WriteDataByIdentifier
    }
}

impl From<WriteDataByIdentifierResponse> for UdsResponse {
    fn from(res: WriteDataByIdentifierResponse) -> Self {
        let service = res.service();
        let payload = res.data_id.to_be_bytes().to_vec();

        UdsResponse::PositiveResponse { service, payload }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_serialization() {
        let req = WriteDataByIdentifierRequest { data_id: 0x1234, data: vec![0xAA, 0xBB] };
        let uds_req: UdsRequest = req.into();
        assert_eq!(uds_req.service, UdsService::WriteDataByIdentifier);
        assert_eq!(uds_req.payload, vec![0x12, 0x34, 0xAA, 0xBB]);
    }

    #[test]
    fn test_request_deserialization() {
        let payload = vec![0x12, 0x34, 0xAA, 0xBB];
        let req = WriteDataByIdentifierRequest::try_from(payload).unwrap();
        assert_eq!(req.data_id, 0x1234);
        assert_eq!(req.data, vec![0xAA, 0xBB]);
    }

    #[test]
    fn test_request_deserialization_payload_too_short() {
        let payload = vec![0x12];
        let result = WriteDataByIdentifierRequest::try_from(payload);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), WriteDataByIdentifierTranslationError::PayloadTooShort);
    }

    #[test]
    fn test_response_serialization() {
        let res = WriteDataByIdentifierResponse { data_id: 0x1234 };
        let uds_res: UdsResponse = res.into();
        match uds_res {
            UdsResponse::PositiveResponse { service, payload } => {
                assert_eq!(service, UdsService::WriteDataByIdentifier);
                assert_eq!(payload, vec![0x12, 0x34]);
            }
            _ => panic!("Unexpected response type"),
        }
    }
}
