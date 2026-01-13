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

/// ReadDataByIdentifier request models.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadDataByIdentifierRequest {
    /// A list of Data Identifiers (DIDs) to be requested.
    /// UDS allows requesting multiple DIDs in a single request (e.g., VIN and ECU Serial).
    pub data_ids: Vec<u16>,
}

/// ReadDataByIdentifier response models.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadDataByIdentifierResponse {
    /// The response contains a list of DID + Data pairs.
    /// The order usually matches the order requested in the Request.
    pub data_records: Vec<DataRecord>,
}

/// A helper struct to represent a single DID and its associated data payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataRecord {
    /// Data identifier (DID)
    pub data_id: u16,
    /// The raw data payload for this specific DID
    pub data: Vec<u8>,
}

/// A builder for a ReadDataByIdentifier response.
#[derive(Default)]
pub struct ReadDataByIdentifierResponseBuilder {
    records: Vec<DataRecord>,
}

// -- Request

/// ReadDataByIdentifier translation errors
#[derive(Debug, PartialEq, thiserror::Error)]
pub enum ReadDataByIdentifierTranslationError {
    /// Unsupported payload length.
    #[error(
        "Unsupported payload length for ReadDataByIdentifier: expected multiple of 2, got {0}."
    )]
    UnsupportedPayloadLength(usize),
    /// Payload cannot be empty.
    #[error("Payload cannot be empty.")]
    EmptyPayload,
}

impl UdsServiceRequest for ReadDataByIdentifierRequest {
    fn service(&self) -> UdsService {
        UdsService::ReadDataByIdentifier
    }
}

impl From<ReadDataByIdentifierRequest> for UdsRequest {
    fn from(req: ReadDataByIdentifierRequest) -> Self {
        let payload: Vec<u8> = req.data_ids.iter().flat_map(|id| id.to_be_bytes()).collect();

        UdsRequest { service: req.service(), payload }
    }
}

impl TryFrom<Vec<u8>> for ReadDataByIdentifierRequest {
    type Error = ReadDataByIdentifierTranslationError;

    fn try_from(payload: Vec<u8>) -> Result<Self, Self::Error> {
        if payload.is_empty() {
            return Err(Self::Error::EmptyPayload);
        }

        // UDS Spec: Payload must be a sequence of 2-byte identifiers
        if !payload.len().is_multiple_of(2) {
            return Err(Self::Error::UnsupportedPayloadLength(payload.len()));
        }

        let data_ids: Vec<u16> = payload
            .chunks_exact(2)
            .map(|chunk| {
                let bytes: [u8; 2] = [chunk[0], chunk[1]];
                u16::from_be_bytes(bytes)
            })
            .collect();

        Ok(ReadDataByIdentifierRequest { data_ids })
    }
}

// -- Response

impl UdsServiceResponse for ReadDataByIdentifierResponse {
    fn service(&self) -> UdsService {
        UdsService::ReadDataByIdentifier
    }
}

impl From<ReadDataByIdentifierResponse> for UdsResponse {
    fn from(res: ReadDataByIdentifierResponse) -> Self {
        let service = res.service();
        let mut bytes = Vec::new();

        for record in res.data_records {
            // Append DID (2 bytes)
            bytes.extend_from_slice(&record.data_id.to_be_bytes());
            // Append Data
            bytes.extend(record.data);
        }

        UdsResponse::PositiveResponse { service, payload: bytes }
    }
}

// -- Builder

impl ReadDataByIdentifierResponseBuilder {
    /// Create a new ReadDataByIdentifier response builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a data record for a specific DID.
    pub fn add_record(mut self, data_id: u16, data: impl Into<Vec<u8>>) -> Self {
        self.records.push(DataRecord { data_id, data: data.into() });
        self
    }

    /// Consumes the builder and returns the final Response.
    pub fn build(self) -> ReadDataByIdentifierResponse {
        ReadDataByIdentifierResponse { data_records: self.records }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- Request Tests --

    #[test]
    fn test_request_serialization_single_did() {
        let req = ReadDataByIdentifierRequest { data_ids: vec![0xF190] };
        let uds_req: UdsRequest = req.into();

        assert_eq!(uds_req.service, UdsService::ReadDataByIdentifier);
        // 0xF190 -> [0xF1, 0x90]
        assert_eq!(uds_req.payload, vec![0xF1, 0x90]);
    }

    #[test]
    fn test_request_serialization_multiple_dids() {
        let req = ReadDataByIdentifierRequest { data_ids: vec![0xF190, 0xF188] };
        let uds_req: UdsRequest = req.into();

        // 0xF190, 0xF188 -> [0xF1, 0x90, 0xF1, 0x88]
        assert_eq!(uds_req.payload, vec![0xF1, 0x90, 0xF1, 0x88]);
    }

    #[test]
    fn test_request_deserialization_valid() {
        let payload = vec![0xF1, 0x90, 0x12, 0x34];
        let req = ReadDataByIdentifierRequest::try_from(payload).unwrap();

        assert_eq!(req.data_ids.len(), 2);
        assert_eq!(req.data_ids[0], 0xF190);
        assert_eq!(req.data_ids[1], 0x1234);
    }

    #[test]
    fn test_request_deserialization_invalid_length() {
        // 3 bytes (odd length) is invalid for UDS 0x22
        let payload = vec![0xF1, 0x90, 0x12];
        let err = ReadDataByIdentifierRequest::try_from(payload).unwrap_err();

        assert!(matches!(err, ReadDataByIdentifierTranslationError::UnsupportedPayloadLength(3)));
    }

    #[test]
    fn test_request_deserialization_empty() {
        let payload = vec![];
        let err = ReadDataByIdentifierRequest::try_from(payload).unwrap_err();

        assert!(matches!(err, ReadDataByIdentifierTranslationError::EmptyPayload));
    }

    // -- Response Tests --

    #[test]
    fn test_response_builder_construction() {
        // Test that the builder correctly assembles the internal struct
        let response = ReadDataByIdentifierResponseBuilder::new()
            .add_record(0x1234, vec![0xAA])
            .add_record(0x5678, vec![0xBB, 0xCC])
            .build();

        assert_eq!(response.data_records.len(), 2);

        // Verify Record 1
        assert_eq!(response.data_records[0].data_id, 0x1234);
        assert_eq!(response.data_records[0].data, vec![0xAA]);

        // Verify Record 2
        assert_eq!(response.data_records[1].data_id, 0x5678);
        assert_eq!(response.data_records[1].data, vec![0xBB, 0xCC]);
    }

    #[test]
    fn test_response_serialization() {
        // Test that the struct converts to the correct UDS byte stream
        let response = ReadDataByIdentifierResponse {
            data_records: vec![
                DataRecord { data_id: 0x1234, data: vec![0xAA] },
                DataRecord { data_id: 0x5678, data: vec![0xBB, 0xCC] },
            ],
        };

        let uds_res: UdsResponse = response.into();

        match uds_res {
            UdsResponse::PositiveResponse { service, payload } => {
                assert_eq!(service, UdsService::ReadDataByIdentifier);

                // Expected payload structure:
                // [DID 1 (2 bytes)] + [Data 1] + [DID 2 (2 bytes)] + [Data 2]
                // [0x12, 0x34]      + [0xAA]   + [0x56, 0x78]      + [0xBB, 0xCC]
                let expected = vec![0x12, 0x34, 0xAA, 0x56, 0x78, 0xBB, 0xCC];
                assert_eq!(payload, expected);
            }
            _ => panic!("Expected PositiveResponse, got {:?}", uds_res),
        }
    }
}
