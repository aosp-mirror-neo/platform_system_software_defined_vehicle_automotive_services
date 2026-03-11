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

use crate::unit_types::{Method, ReadDataItem, WriteDataItem};
use async_trait::async_trait;
use diagnostics_v1_api::data_item_service::{
    ReadDataItemRequest, ReadDataItemResponse, WriteDataItemRequest, WriteDataItemResponse,
};
use futures::StreamExt;
use log::info;
use protobuf::reflect::MessageDescriptor;
use protobuf::Message;
use protobuf::MessageFull;
use sdv_comms::id::ServiceFqin;
use sdv_comms::rpc::{Client, ClientOptions};
use sdv_comms::sd::{ServiceDiscoveryManager, ServiceUnitDefinition, UnitNameDiscoveryArgs};
use sdv_comms::ContextRef;
use sdv_config_provider::vsidl_provider_api::VsidlConfigProviderV1;
use sdv_status_rs::{SdvResult, SdvStatus, SdvStatusCode};
use std::sync::Arc;
use tokio::sync::OnceCell;

const COMPLIANCE_PACKAGE: &str = "com.android.sdv.compliance.diagnostics";
const COMPLIANCE_BUNDLE: &str = "DiagnosticsProvider";

/// A client for interacting with the Compliance Service Bundle via the SDV Diagnostics API.
#[async_trait]
pub trait ComplianceClient: Send + Sync {
    /// Fetches a data item by its unique ID.
    async fn read_data_item(&self, identifier: &str) -> SdvResult<ReadDataItemResponse>;

    /// Writes a data item by its unique ID.
    async fn write_data_item(
        &self,
        identifier: &str,
        value: Vec<u8>,
    ) -> SdvResult<WriteDataItemResponse>;
}

/// Proxy to wrap an inner lazy loaded SdvComplianceClient.
pub struct ProxyComplianceClient {
    context: ContextRef,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
    inner_client: OnceCell<SdvComplianceClient>,
}

/// Actual ComplianceClient implementation with async dependencies.
pub struct SdvComplianceClient {
    rpc_client: Client,
    service_unit: ServiceUnitDefinition,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
}

// -- SdvComplianceClient

impl SdvComplianceClient {
    /// Discovers the diagnostics Compliance Service Bundle and establishes an RPC client.
    /// Returns a SdvComplianceClient instance on success.
    pub async fn connect(
        context: ContextRef,
        vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
    ) -> SdvResult<Self> {
        // Discover the Service Unit
        let name_args = UnitNameDiscoveryArgs::new_builder()
            .set_sdv_package_name(COMPLIANCE_PACKAGE)
            .set_service_bundle_name(COMPLIANCE_BUNDLE)
            .build()
            .map_err(|e| {
                SdvStatus::with_message(
                    SdvStatusCode::Internal,
                    format!("UnitNameDiscoveryArgs build failed: {e:?}"),
                )
            })?;

        let sd_manager = ServiceDiscoveryManager::new(context);
        let service_unit =
            sd_manager.list_units_by_name(&name_args).next().await.ok_or_else(|| {
                SdvStatus::with_message(
                    SdvStatusCode::NotFound,
                    format!("Service not found: {}.{}", COMPLIANCE_PACKAGE, COMPLIANCE_BUNDLE),
                )
            })?;

        // Create the RPC Client
        let rpc_client =
            Client::create(context, service_unit.get_unit_identifier(), &ClientOptions::default())
                .await?;

        info!("ComplianceClient connected successfully.");
        Ok(Self { rpc_client, service_unit, vsidl_provider })
    }

    /// Get the Compliance Service Bundle's fully qualified instance name.
    fn service_bundle_fqin(&self) -> ServiceFqin {
        self.service_unit.get_service_bundle_identity().get_fqin().clone()
    }

    fn decode<T: MessageFull>(message_bytes: &[u8]) -> Result<T, protobuf::Error> {
        T::parse_from_bytes(message_bytes)
    }

    async fn get_message_name(&self, id: &str) -> SdvResult<String> {
        let service_fqin = self.service_bundle_fqin();
        let diagnostics_declaration =
            self.vsidl_provider.get_diagnostics_declaration(&service_fqin).await.ok_or_else(
                || {
                    SdvStatus::with_message(
                        SdvStatusCode::NotFound,
                        format!("Diagnostics declaration not found for {service_fqin}"),
                    )
                },
            )?;
        let item = diagnostics_declaration.data_item.iter().find(|item| item.id == id).ok_or_else(
            || {
                SdvStatus::with_message(
                    SdvStatusCode::NotFound,
                    format!("DataItem {id} not found in diagnostics declaration"),
                )
            },
        )?;

        Ok(item.message_name.clone())
    }

    async fn get_message_descriptor(&self, id: &str) -> SdvResult<(MessageDescriptor, String)> {
        let message_name = self.get_message_name(id).await?;
        let service_fqin = self.service_bundle_fqin();
        let descriptor = self
            .vsidl_provider
            .get_message_descriptor(&service_fqin, &message_name)
            .await
            .ok_or_else(|| {
                SdvStatus::with_message(
                    SdvStatusCode::NotFound,
                    format!("Message descriptor not found: {message_name}"),
                )
            })?;
        Ok((descriptor, message_name))
    }

    async fn validate_payload(&self, id: &str, payload: &[u8]) -> SdvResult<()> {
        let (descriptor, message_name) = self.get_message_descriptor(id).await?;

        descriptor.parse_from_bytes(payload).map_err(|e| {
            SdvStatus::with_message(
                SdvStatusCode::InvalidArgument,
                format!("Invalid proto bytes for {message_name}: {e:?}"),
            )
        })?;

        Ok(())
    }
}

#[async_trait]
impl ComplianceClient for SdvComplianceClient {
    async fn read_data_item(&self, id: &str) -> SdvResult<ReadDataItemResponse> {
        info!("Reading DataItem: {id}");

        let request = ReadDataItemRequest { id: id.to_string(), ..Default::default() };

        let encoded_req = request.write_to_bytes().map_err(|e| {
            SdvStatus::with_message(
                SdvStatusCode::Internal,
                format!("Failed to encode request: {e:?}"),
            )
        })?;

        let response_bytes =
            self.rpc_client.unary_call(ReadDataItem::METHOD_NAME, &encoded_req).await?;

        let response = Self::decode::<ReadDataItemResponse>(&response_bytes).map_err(|e| {
            SdvStatus::with_message(
                SdvStatusCode::Internal,
                format!("Failed to decode response: {e:?}"),
            )
        })?;

        self.validate_payload(id, &response.value).await?;

        Ok(response)
    }

    async fn write_data_item(&self, id: &str, value: Vec<u8>) -> SdvResult<WriteDataItemResponse> {
        info!("Writing DataItem: {id}");

        self.validate_payload(id, &value).await?;

        let request = WriteDataItemRequest { id: id.to_string(), value, ..Default::default() };

        let encoded_req = request.write_to_bytes().map_err(|e| {
            SdvStatus::with_message(
                SdvStatusCode::Internal,
                format!("Failed to encode request: {e:?}"),
            )
        })?;

        let response_bytes =
            self.rpc_client.unary_call(WriteDataItem::METHOD_NAME, &encoded_req).await?;

        Self::decode::<WriteDataItemResponse>(&response_bytes).map_err(|e| {
            SdvStatus::with_message(
                SdvStatusCode::Internal,
                format!("Failed to decode response: {e:?}"),
            )
        })
    }
}

// -- Proxy

impl ProxyComplianceClient {
    pub fn new(context: ContextRef, vsidl_provider: Arc<dyn VsidlConfigProviderV1>) -> Self {
        Self { context, vsidl_provider, inner_client: OnceCell::new() }
    }

    async fn get_inner_client(&self) -> SdvResult<&SdvComplianceClient> {
        self.inner_client
            .get_or_try_init(|| async {
                SdvComplianceClient::connect(self.context, self.vsidl_provider.clone()).await
            })
            .await
    }
}

#[async_trait]
impl ComplianceClient for ProxyComplianceClient {
    async fn read_data_item(&self, identifier: &str) -> SdvResult<ReadDataItemResponse> {
        self.get_inner_client().await?.read_data_item(identifier).await
    }

    async fn write_data_item(
        &self,
        identifier: &str,
        value: Vec<u8>,
    ) -> SdvResult<WriteDataItemResponse> {
        self.get_inner_client().await?.write_data_item(identifier, value).await
    }
}
