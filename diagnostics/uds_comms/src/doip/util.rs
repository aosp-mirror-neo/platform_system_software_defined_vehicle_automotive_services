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

use doip_definitions::header::{DoipHeader, PayloadType, ProtocolVersion};
use doip_definitions::message::DoipMessage;
use doip_definitions::payload::DoipPayload;

/// Returns a `DoipMessage` including a `DoipHeader` and the provided `DoipPayload`.
pub fn build_message(payload: DoipPayload) -> DoipMessage {
    let payload_bytes: Vec<u8> = payload.clone().into();
    let payload_length = payload_bytes.len() as u32;

    let payload_type = payload.payload_type();
    let protocol_version = ProtocolVersion::Iso13400_2019;

    DoipMessage {
        header: DoipHeader {
            protocol_version,
            inverse_protocol_version: !(protocol_version as u8),
            payload_type,
            payload_length,
        },
        payload,
    }
}

/// An extension trait for `DoipPayload` to get its corresponding `PayloadType`.
pub trait DoipPayloadExt {
    /// Returns the `PayloadType` enum variant corresponding to the `DoipPayload` variant.
    fn payload_type(&self) -> PayloadType;
}

impl DoipPayloadExt for DoipPayload {
    fn payload_type(&self) -> PayloadType {
        match self {
            DoipPayload::GenericNack(_) => PayloadType::GenericNack,
            DoipPayload::VehicleIdentificationRequest(_) => {
                PayloadType::VehicleIdentificationRequest
            }
            DoipPayload::VehicleIdentificationRequestEid(_) => {
                PayloadType::VehicleIdentificationRequestEid
            }
            DoipPayload::VehicleIdentificationRequestVin(_) => {
                PayloadType::VehicleIdentificationRequestVin
            }
            DoipPayload::VehicleAnnouncementMessage(_) => PayloadType::VehicleAnnouncementMessage,
            DoipPayload::RoutingActivationRequest(_) => PayloadType::RoutingActivationRequest,
            DoipPayload::RoutingActivationResponse(_) => PayloadType::RoutingActivationResponse,
            DoipPayload::AliveCheckRequest(_) => PayloadType::AliveCheckRequest,
            DoipPayload::AliveCheckResponse(_) => PayloadType::AliveCheckResponse,
            DoipPayload::EntityStatusRequest(_) => PayloadType::EntityStatusRequest,
            DoipPayload::EntityStatusResponse(_) => PayloadType::EntityStatusResponse,
            DoipPayload::PowerInformationRequest(_) => PayloadType::PowerInformationRequest,
            DoipPayload::PowerInformationResponse(_) => PayloadType::PowerInformationResponse,
            DoipPayload::DiagnosticMessage(_) => PayloadType::DiagnosticMessage,
            DoipPayload::DiagnosticMessageAck(_) => PayloadType::DiagnosticMessageAck,
            DoipPayload::DiagnosticMessageNack(_) => PayloadType::DiagnosticMessageNack,
        }
    }
}
