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

/// Server for inbound UDS communication over DoIP.
pub mod server;

/// Maps UDS data requests to the Compliance Service Bundle.
mod data_item_service;

/// Abstraction over the SDV Diagnostics API to enable communication with the Compliance Service Bundle.
mod compliance_client;

use crate::uds::compliance_client::ProxyComplianceClient;
use crate::uds::data_item_service::{DataItemRequestHandler, DataItemService};
use sdv_comms::ContextRef;
use sdv_config_provider::vsidl_provider_api::VsidlConfigProviderV1;
use sdv_uds_comms::doip::listener::DoipListenerConfig;
use sdv_uds_comms::doip::logical_address::LogicalAddress;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;

static DEFAULT_DOIP_PORT: u16 = 13400;
static LOCALHOST_IP_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0));
static ECU_LOGICAL_ADDR: u16 = 0x1000;
static FALLBACK_VIN: [u8; 17] = [0x30; 17];
static DEFAULT_GOOGLE_RESERVED_DID_RANGE_START: u16 = 0xF0F0;

/// Creates a default config for the `DoipListener`.
pub fn create_doip_config() -> DoipListenerConfig {
    DoipListenerConfig {
        socket_addr: SocketAddr::new(LOCALHOST_IP_ADDR, DEFAULT_DOIP_PORT),
        logical_addr: LogicalAddress::new(ECU_LOGICAL_ADDR),
        vin: FALLBACK_VIN,
    }
}

/// Creates a service for communication with the Compliance Service Bundle via comms stack.
pub fn create_data_item_service(
    context: ContextRef,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
) -> Box<dyn DataItemRequestHandler> {
    let compliance_client = ProxyComplianceClient::new(context, vsidl_provider);
    let data_item_service =
        DataItemService::new(Box::new(compliance_client), DEFAULT_GOOGLE_RESERVED_DID_RANGE_START);
    Box::new(data_item_service)
}
