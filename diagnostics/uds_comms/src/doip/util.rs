// Copyright 2025 Google LLC

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
