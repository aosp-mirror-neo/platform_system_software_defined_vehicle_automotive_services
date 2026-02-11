// Copyright (C) 2026 The Android Open Source Project
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::uds::compliance_client::ComplianceClient;
use async_trait::async_trait;
use diagnostics_v1_api::response_code::ResponseCode;
use log::{error, warn};
use sdv_diag_compliance_id::DiagComplianceId;
use sdv_uds_comms::models::request::UdsRequest;
use sdv_uds_comms::models::response::{NegativeResponseCode, RequestedService, UdsResponse};
use sdv_uds_comms::models::services::{
    DataRecord, ReadDataByIdentifierRequest, ReadDataByIdentifierResponseBuilder,
    UdsServiceRequest, WriteDataByIdentifierRequest, WriteDataByIdentifierResponse,
};

/// Trait for handling UDS data item requests.
#[async_trait]
pub trait DataItemRequestHandler: Send + Sync {
    async fn read_data(&self, request: UdsRequest) -> UdsResponse;
    async fn write_data(&self, request: UdsRequest) -> UdsResponse;
}

/// Maps UDS requests to the Compliance Service Bundle.
pub struct DataItemService {
    compliance_client: Box<dyn ComplianceClient>,
}

impl DataItemService {
    pub fn new(compliance_client: Box<dyn ComplianceClient>) -> Self {
        Self { compliance_client }
    }

    async fn read_data_by_id(&self, data_id: u16) -> Result<DataRecord, NegativeResponseCode> {
        let identifier = DiagComplianceId::from_u16(data_id)
            .ok_or(NegativeResponseCode::RequestOutOfRange)?
            .to_string();

        let data_item = self.compliance_client.read_data_item(&identifier).await.map_err(|e| {
            error!("Read failed for DID {:#06X} (id: {}): {:?}", data_id, identifier, e);
            NegativeResponseCode::GeneralReject
        })?;

        let uds_bytes =
            self.compliance_client.translate_to_uds(&identifier, &data_item).await.map_err(
                |e| {
                    error!(
                        "UDS translation failed for DID {:#06X} (id: {}): {:?}",
                        data_id, identifier, e
                    );
                    NegativeResponseCode::GeneralReject
                },
            )?;

        Ok(DataRecord { data_id, data: uds_bytes })
    }

    async fn write_data_by_id(
        &self,
        data_id: u16,
        data: &[u8],
    ) -> Result<(), NegativeResponseCode> {
        let identifier = DiagComplianceId::from_u16(data_id)
            .ok_or(NegativeResponseCode::RequestOutOfRange)?
            .to_string();

        let proto_bytes =
            self.compliance_client.translate_from_uds(&identifier, data).await.map_err(|e| {
                error!(
                    "UDS translation failed for DID {:#06X} (id: {}): {:?}",
                    data_id, identifier, e
                );
                NegativeResponseCode::GeneralReject
            })?;

        let response =
            self.compliance_client.write_data_item(&identifier, proto_bytes).await.map_err(
                |e| {
                    error!("Write failed for DID {:#06X} (id: {}): {:?}", data_id, identifier, e);
                    NegativeResponseCode::GeneralReject
                },
            )?;

        match response.response_code.enum_value_or_default() {
            ResponseCode::POSITIVE_RESPONSE => Ok(()),
            _ => Err(NegativeResponseCode::GeneralReject),
        }
    }
}

#[async_trait]
impl DataItemRequestHandler for DataItemService {
    async fn read_data(&self, request: UdsRequest) -> UdsResponse {
        let request = match ReadDataByIdentifierRequest::try_from(request.payload) {
            Ok(req) => req,
            Err(err) => {
                warn!("Could not decode ReadDataByIdentifier request: {:?}", err);
                return UdsResponse::NegativeResponse {
                    service: RequestedService::Known(request.service),
                    code: NegativeResponseCode::IncorrectMessageLengthOrInvalidFormat,
                };
            }
        };

        let mut builder = ReadDataByIdentifierResponseBuilder::new();

        for &data_id in request.data_ids.iter() {
            match self.read_data_by_id(data_id).await {
                Ok(record) => {
                    builder.add_record(record);
                }
                Err(negative_response_code) => {
                    // If any data_id fails the server discards all data and returns a negative response.
                    return UdsResponse::NegativeResponse {
                        service: RequestedService::Known(request.service()),
                        code: negative_response_code,
                    };
                }
            }
        }

        let response = builder.build();
        response.into()
    }

    async fn write_data(&self, request: UdsRequest) -> UdsResponse {
        let request = match WriteDataByIdentifierRequest::try_from(request.payload) {
            Ok(req) => req,
            Err(err) => {
                warn!("Could not decode WriteDataByIdentifier request: {:?}", err);
                return UdsResponse::NegativeResponse {
                    service: RequestedService::Known(request.service),
                    code: NegativeResponseCode::IncorrectMessageLengthOrInvalidFormat,
                };
            }
        };

        match self.write_data_by_id(request.data_id, &request.data).await {
            Ok(_) => WriteDataByIdentifierResponse { data_id: request.data_id }.into(),
            Err(code) => UdsResponse::NegativeResponse {
                service: RequestedService::Known(request.service()),
                code,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use diagnostics_v1_api::data_item_service::{ReadDataItemResponse, WriteDataItemResponse};
    use sdv_status_rs::{SdvResult, SdvStatus, SdvStatusCode};
    use sdv_uds_comms::models::request::UdsService;
    use std::collections::HashMap;

    #[derive(Default)]
    struct MockComplianceClient {
        read_results: HashMap<String, ReadDataItemResponse>,
        proto_to_uds_translate_results: HashMap<String, Vec<u8>>,
        uds_to_proto_translate_results: HashMap<String, Vec<u8>>,
        write_results: HashMap<String, ResponseCode>,
    }

    #[async_trait]
    impl ComplianceClient for MockComplianceClient {
        /// Returns registered result for identifier or error if not found
        async fn read_data_item(&self, identifier: &str) -> SdvResult<ReadDataItemResponse> {
            match self.read_results.get(identifier) {
                Some(result) => Ok(result.clone()),
                None => Err(SdvStatus::new(SdvStatusCode::Unknown)),
            }
        }

        async fn write_data_item(
            &self,
            identifier: &str,
            _value: Vec<u8>,
        ) -> SdvResult<WriteDataItemResponse> {
            match self.write_results.get(identifier) {
                Some(code) => Ok(WriteDataItemResponse {
                    response_code: ::protobuf::EnumOrUnknown::new(*code),
                    ..Default::default()
                }),
                None => Err(SdvStatus::new(SdvStatusCode::Unknown)),
            }
        }

        async fn translate_to_uds(
            &self,
            identifier: &str,
            _data_item: &ReadDataItemResponse,
        ) -> SdvResult<Vec<u8>> {
            match self.proto_to_uds_translate_results.get(identifier) {
                Some(result) => Ok(result.clone()),
                None => Err(SdvStatus::new(SdvStatusCode::Unknown)),
            }
        }

        async fn translate_from_uds(
            &self,
            identifier: &str,
            _uds_bytes: &[u8],
        ) -> SdvResult<Vec<u8>> {
            match self.uds_to_proto_translate_results.get(identifier) {
                Some(result) => Ok(result.clone()),
                None => Err(SdvStatus::new(SdvStatusCode::Unknown)),
            }
        }
    }

    #[tokio::test]
    async fn test_read_data_by_id_out_of_range() {
        let client = MockComplianceClient::default();
        let service = DataItemService::new(Box::new(client));
        let result = service.read_data_by_id(0x1234).await;
        assert_eq!(result, Err(NegativeResponseCode::RequestOutOfRange));
    }

    #[tokio::test]
    async fn test_read_data_by_id_read_fails() {
        let client = MockComplianceClient::default();
        let service = DataItemService::new(Box::new(client));
        let result = service.read_data_by_id(0xF0F0).await;
        assert_eq!(result, Err(NegativeResponseCode::GeneralReject));
    }

    #[tokio::test]
    async fn test_read_data_by_id_translate_fails() {
        let mut client = MockComplianceClient::default();
        client
            .read_results
            .insert(DiagComplianceId::Reserved0.to_string(), ReadDataItemResponse::new());
        let service = DataItemService::new(Box::new(client));
        let result = service.read_data_by_id(0xF0F0).await;
        assert_eq!(result, Err(NegativeResponseCode::GeneralReject));
    }

    #[tokio::test]
    async fn test_read_data_by_id_success() {
        let mut client = MockComplianceClient::default();
        client
            .read_results
            .insert(DiagComplianceId::Reserved0.to_string(), ReadDataItemResponse::new());
        client
            .proto_to_uds_translate_results
            .insert(DiagComplianceId::Reserved0.to_string(), vec![1, 2, 3]);
        let service = DataItemService::new(Box::new(client));
        let result = service.read_data_by_id(0xF0F0).await;
        assert_eq!(result, Ok(DataRecord { data_id: 0xF0F0, data: vec![1, 2, 3] }));
    }

    #[tokio::test]
    async fn test_read_data_invalid_payload() {
        let client = MockComplianceClient::default();
        let service = DataItemService::new(Box::new(client));
        let request = UdsRequest {
            service: UdsService::ReadDataByIdentifier,
            payload: vec![0x12], // Invalid length
        };
        let response = service.read_data(request).await;
        assert_eq!(
            response,
            UdsResponse::NegativeResponse {
                service: RequestedService::Known(UdsService::ReadDataByIdentifier),
                code: NegativeResponseCode::IncorrectMessageLengthOrInvalidFormat,
            }
        );
    }

    #[tokio::test]
    async fn test_read_data_one_id_fails() {
        let mut client = MockComplianceClient::default();
        // F0F0 will succeed
        client
            .read_results
            .insert(DiagComplianceId::Reserved0.to_string(), ReadDataItemResponse::new());
        client
            .proto_to_uds_translate_results
            .insert(DiagComplianceId::Reserved0.to_string(), vec![1, 2, 3]);
        // F0F1 will fail on read
        // no value inserted for the corresponding identifier

        let service = DataItemService::new(Box::new(client));
        let req = ReadDataByIdentifierRequest { data_ids: vec![0xF0F0, 0xF0F1] };
        let response = service.read_data(req.into()).await;
        assert_eq!(
            response,
            UdsResponse::NegativeResponse {
                service: RequestedService::Known(UdsService::ReadDataByIdentifier),
                code: NegativeResponseCode::GeneralReject,
            }
        );
    }

    #[tokio::test]
    async fn test_read_data_success() {
        let mut client = MockComplianceClient::default();
        client
            .read_results
            .insert(DiagComplianceId::Reserved0.to_string(), ReadDataItemResponse::new());
        client
            .proto_to_uds_translate_results
            .insert(DiagComplianceId::Reserved0.to_string(), vec![1, 2, 3]);

        let service = DataItemService::new(Box::new(client));
        let req = ReadDataByIdentifierRequest { data_ids: vec![0xF0F0] };
        let response = service.read_data(req.into()).await;

        let mut builder = ReadDataByIdentifierResponseBuilder::new();
        builder.add_record(DataRecord { data_id: 0xF0F0, data: vec![1, 2, 3] });
        let expected_response: UdsResponse = builder.build().into();

        assert_eq!(response, expected_response);
    }

    #[tokio::test]
    async fn test_write_data_success() {
        let mut client = MockComplianceClient::default();
        client
            .write_results
            .insert(DiagComplianceId::Reserved0.to_string(), ResponseCode::POSITIVE_RESPONSE);
        client
            .uds_to_proto_translate_results
            .insert(DiagComplianceId::Reserved0.to_string(), vec![1, 2, 3]);

        let service = DataItemService::new(Box::new(client));
        let req = WriteDataByIdentifierRequest { data_id: 0xF0F0, data: vec![1, 2, 3] };
        let response = service.write_data(req.into()).await;

        let expected_response: UdsResponse =
            WriteDataByIdentifierResponse { data_id: 0xF0F0 }.into();

        assert_eq!(response, expected_response);
    }

    #[tokio::test]
    async fn test_write_data_failure() {
        let mut client = MockComplianceClient::default();
        client
            .write_results
            .insert(DiagComplianceId::Reserved0.to_string(), ResponseCode::CONDITIONS_NOT_CORRECT);
        client
            .uds_to_proto_translate_results
            .insert(DiagComplianceId::Reserved0.to_string(), vec![1, 2, 3]);

        let service = DataItemService::new(Box::new(client));
        let req = WriteDataByIdentifierRequest { data_id: 0xF0F0, data: vec![1, 2, 3] };
        let response = service.write_data(req.into()).await;

        assert_eq!(
            response,
            UdsResponse::NegativeResponse {
                service: RequestedService::Known(UdsService::WriteDataByIdentifier),
                code: NegativeResponseCode::GeneralReject,
            }
        );
    }

    #[tokio::test]
    async fn test_write_data_out_of_range() {
        let service = DataItemService::new(Box::new(MockComplianceClient::default()));
        let result = service.write_data_by_id(0x1234, &[]).await;
        assert_eq!(result, Err(NegativeResponseCode::RequestOutOfRange));
    }

    #[tokio::test]
    async fn test_write_data_by_id_translate_from_uds_fails() {
        let client = MockComplianceClient::default();
        let service = DataItemService::new(Box::new(client));
        let result = service.write_data_by_id(0xF0F0, &[1, 2, 3]).await;

        assert_eq!(result, Err(NegativeResponseCode::GeneralReject));
    }
}
