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

use crate::unit_types::{Method, ReadDataItem};
use async_trait::async_trait;
use diagnostics_v1_api::data_item_service::{ReadDataItemRequest, ReadDataItemResponse};
use futures::StreamExt;
use log::{debug, info};
use protobuf::{Message, MessageFull};
use sdv_comms::id::ServiceFqin;
use sdv_comms::rpc::{Client, ClientOptions};
use sdv_comms::sd::{ServiceDiscoveryManager, ServiceUnitDefinition, UnitNameDiscoveryArgs};
use sdv_comms::ContextRef;
use sdv_config_provider::vsidl_provider_api::VsidlConfigProviderV1;
use sdv_status_rs::{SdvResult, SdvStatus, SdvStatusCode};
use sdv_uds_serde::{diagnostics_translator::DiagnosticsTranslator, entity_id::*};
use std::sync::Arc;
use tokio::sync::OnceCell;

const COMPLIANCE_PACKAGE: &str = "com.sdv.google.compliance.diagnostics";
const COMPLIANCE_BUNDLE: &str = "DiagnosticsProvider";

/// A client for interacting with the Compliance Service Bundle via the SDV Diagnostics API.
#[async_trait]
pub trait ComplianceClient: Send + Sync {
    /// Fetches a data item by its unique ID.
    async fn read_data_item(&self, identifier: &str) -> SdvResult<ReadDataItemResponse>;

    /// Translates a DataItem value to UDS byte format.
    async fn translate_to_uds(
        &self,
        identifier: &str,
        data_item: &ReadDataItemResponse,
    ) -> SdvResult<Vec<u8>>;
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
    translator: DiagnosticsTranslator,
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

        let translator = DiagnosticsTranslator::new(vsidl_provider.clone());

        info!("ComplianceClient connected successfully.");
        Ok(Self { rpc_client, service_unit, translator })
    }

    /// Get the Compliance Service Bundle's fully qualified instance name.
    fn service_bundle_fqin(&self) -> ServiceFqin {
        self.service_unit.get_service_bundle_identity().get_fqin().clone()
    }

    fn decode<T: MessageFull>(message_bytes: &[u8]) -> Result<T, protobuf::Error> {
        T::parse_from_bytes(message_bytes)
    }
}

#[async_trait]
impl ComplianceClient for SdvComplianceClient {
    async fn read_data_item(&self, id: &str) -> SdvResult<ReadDataItemResponse> {
        info!("Reading DataItem: {}", id);

        let request = ReadDataItemRequest { id: id.to_string(), ..Default::default() };

        let encoded_req = request.write_to_bytes().map_err(|e| {
            SdvStatus::with_message(
                SdvStatusCode::Internal,
                format!("Failed to encode request: {e:?}"),
            )
        })?;

        let response_bytes =
            self.rpc_client.unary_call(ReadDataItem::METHOD_NAME, &encoded_req).await?;

        Self::decode::<ReadDataItemResponse>(&response_bytes).map_err(|e| {
            SdvStatus::with_message(
                SdvStatusCode::Internal,
                format!("Failed to decode response: {e:?}"),
            )
        })
    }

    async fn translate_to_uds(
        &self,
        id: &str,
        data_item: &ReadDataItemResponse,
    ) -> SdvResult<Vec<u8>> {
        let service_fqin = self.service_bundle_fqin();

        let translation_result = self
            .translator
            .translate_from_protobuf_to_uds(
                &service_fqin,
                EntityId::DataItem(id.to_owned()),
                &data_item.value,
            )
            .await
            .map_err(|e| {
                SdvStatus::with_message(
                    SdvStatusCode::Internal,
                    format!("Failed to translate DataItem value to UDS: {e:?}"),
                )
            });

        if let Ok(ref uds_bytes) = translation_result {
            debug!(
                "Read DataItem: {}#{}, proto_value: {:?}, uds_value {:?}",
                &service_fqin, id, data_item.value, uds_bytes
            );
        }

        translation_result
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

    async fn translate_to_uds(
        &self,
        identifier: &str,
        data_item: &ReadDataItemResponse,
    ) -> SdvResult<Vec<u8>> {
        self.get_inner_client().await?.translate_to_uds(identifier, data_item).await
    }
}
