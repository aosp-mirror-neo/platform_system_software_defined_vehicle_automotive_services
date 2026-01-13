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

use bytes::{Buf, BytesMut};
use doip_definitions::header::{DoipHeader, PayloadType};
use doip_definitions::message::DoipMessage;
use doip_definitions::payload::{
    AliveCheckRequest, AliveCheckResponse, DiagnosticMessage, DiagnosticMessageAck,
    DiagnosticMessageNack, DoipPayload, EntityStatusRequest, EntityStatusResponse, GenericNack,
    PowerInformationRequest, PowerInformationResponse, RoutingActivationRequest,
    RoutingActivationResponse, VehicleAnnouncementMessage, VehicleIdentificationRequest,
    VehicleIdentificationRequestEid, VehicleIdentificationRequestVin,
};
use tokio_util::codec::{Decoder, Encoder};

const DOIP_HEADER_LENGTH: usize = 8;

/// A tokio-util codec for framing DoIP messages on a TCP stream.
#[derive(Debug, Default)]
pub struct DoipCodec;

/// DoIP codec errors when framing byte sequences.
#[derive(Debug, thiserror::Error)]
pub enum DoipCodecError {
    /// The byte representation of the DoIP header is invalid.
    #[error("Invalid DoIP header: {0}")]
    InvalidHeader(String),
    /// The byte representation of the DoIP pazload is invalid.
    #[error("Invalid DoIP payload: {0}")]
    InvalidPayload(String),
    /// Underlying I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl PartialEq for DoipCodecError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::InvalidHeader(left), Self::InvalidHeader(right)) => left == right,
            (Self::InvalidPayload(left), Self::InvalidPayload(right)) => left == right,
            // std::io::Error does not implement PartialEq, so we compare by kind.
            (Self::Io(left), Self::Io(right)) => left.kind() == right.kind(),
            _ => false,
        }
    }
}

impl DoipCodec {
    /// Parses the payload slice based on the payload type from the header.
    fn parse_payload(
        &self,
        header: &DoipHeader,
        payload_slice: &[u8],
    ) -> Result<DoipPayload, DoipCodecError> {
        let map_err =
            |e: doip_definitions::error::Error| DoipCodecError::InvalidPayload(e.to_string());

        let payload = match header.payload_type {
            PayloadType::AliveCheckRequest => {
                DoipPayload::AliveCheckRequest(AliveCheckRequest::from(payload_slice))
            }
            PayloadType::AliveCheckResponse => DoipPayload::AliveCheckResponse(
                AliveCheckResponse::try_from(payload_slice).map_err(map_err)?,
            ),
            PayloadType::DiagnosticMessage => DoipPayload::DiagnosticMessage(
                DiagnosticMessage::try_from(payload_slice).map_err(map_err)?,
            ),
            PayloadType::DiagnosticMessageAck => DoipPayload::DiagnosticMessageAck(
                DiagnosticMessageAck::try_from(payload_slice).map_err(map_err)?,
            ),
            PayloadType::DiagnosticMessageNack => DoipPayload::DiagnosticMessageNack(
                DiagnosticMessageNack::try_from(payload_slice).map_err(map_err)?,
            ),
            PayloadType::GenericNack => {
                DoipPayload::GenericNack(GenericNack::try_from(payload_slice).map_err(map_err)?)
            }
            PayloadType::VehicleIdentificationRequestEid => {
                DoipPayload::VehicleIdentificationRequestEid(
                    VehicleIdentificationRequestEid::try_from(payload_slice).map_err(map_err)?,
                )
            }
            PayloadType::VehicleIdentificationRequestVin => {
                DoipPayload::VehicleIdentificationRequestVin(
                    VehicleIdentificationRequestVin::try_from(payload_slice).map_err(map_err)?,
                )
            }
            PayloadType::VehicleAnnouncementMessage => DoipPayload::VehicleAnnouncementMessage(
                VehicleAnnouncementMessage::try_from(payload_slice).map_err(map_err)?,
            ),
            PayloadType::VehicleIdentificationRequest => DoipPayload::VehicleIdentificationRequest(
                VehicleIdentificationRequest::from(payload_slice),
            ),
            PayloadType::RoutingActivationRequest => DoipPayload::RoutingActivationRequest(
                RoutingActivationRequest::try_from(payload_slice).map_err(map_err)?,
            ),
            PayloadType::RoutingActivationResponse => DoipPayload::RoutingActivationResponse(
                RoutingActivationResponse::try_from(payload_slice).map_err(map_err)?,
            ),
            PayloadType::EntityStatusRequest => {
                DoipPayload::EntityStatusRequest(EntityStatusRequest::from(payload_slice))
            }
            PayloadType::EntityStatusResponse => DoipPayload::EntityStatusResponse(
                EntityStatusResponse::try_from(payload_slice).map_err(map_err)?,
            ),
            PayloadType::PowerInformationRequest => {
                DoipPayload::PowerInformationRequest(PowerInformationRequest::from(payload_slice))
            }
            PayloadType::PowerInformationResponse => DoipPayload::PowerInformationResponse(
                PowerInformationResponse::try_from(payload_slice).map_err(map_err)?,
            ),
        };
        Ok(payload)
    }
}

impl Decoder for DoipCodec {
    type Item = DoipMessage;
    type Error = DoipCodecError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < DOIP_HEADER_LENGTH {
            // Not enough data for a header, ask for more
            return Ok(None);
        }

        let header_slice: [u8; DOIP_HEADER_LENGTH] = src[..DOIP_HEADER_LENGTH]
            .try_into()
            .expect("Buffer length is guaranteed to be at least DOIP_HEADER_LENGTH");

        let header = DoipHeader::try_from(header_slice)
            .map_err(|e| DoipCodecError::InvalidHeader(e.to_string()))?;
        let payload_length = header.payload_length as usize;
        let total_length = DOIP_HEADER_LENGTH + payload_length;

        if src.len() < total_length {
            // We have the header, but not the full payload yet
            src.reserve(total_length - src.len());
            return Ok(None);
        }

        // Parse the payload
        let payload_slice: &[u8] = &src[DOIP_HEADER_LENGTH..total_length];
        let payload = self.parse_payload(&header, payload_slice)?;

        // Advance buffer to mark bytes as processed
        let advance_length = if src.remaining() >= total_length { total_length } else { src.len() };
        src.advance(advance_length);

        Ok(Some(DoipMessage { header, payload }))
    }
}

impl Encoder<DoipMessage> for DoipCodec {
    type Error = DoipCodecError;

    fn encode(&mut self, item: DoipMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let payload_length = item.header.payload_length as usize;
        let header: [u8; DOIP_HEADER_LENGTH] = item.header.into();
        let payload: Vec<u8> = item.payload.into();

        dst.reserve(DOIP_HEADER_LENGTH + payload_length);
        dst.extend_from_slice(&header);
        dst.extend_from_slice(&payload);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use doip_definitions::header::ProtocolVersion;
    use doip_definitions::payload::{
        ActivationCode, ActivationType, RoutingActivationRequest, RoutingActivationResponse,
    };
    use googletest::{
        assert_that,
        matchers::{err, matches_pattern},
    };

    fn create_routing_activation_request() -> DoipMessage {
        let payload = DoipPayload::RoutingActivationRequest(RoutingActivationRequest {
            source_address: 0x0E00_u16.to_be_bytes(),
            activation_type: ActivationType::Default,
            buffer: [0; 4],
        });
        let payload_bytes: Vec<u8> = payload.clone().into();
        DoipMessage {
            header: DoipHeader {
                protocol_version: ProtocolVersion::Iso13400_2019,
                inverse_protocol_version: !(ProtocolVersion::Iso13400_2019 as u8),
                payload_type: PayloadType::RoutingActivationRequest,
                payload_length: payload_bytes.len() as u32,
            },
            payload,
        }
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let mut codec = DoipCodec {};
        let message = create_routing_activation_request();

        // Encode
        let mut buffer = BytesMut::new();
        codec.encode(message.clone(), &mut buffer).expect("Encoding failed");

        // Decode
        let decoded_message =
            codec.decode(&mut buffer).expect("Decoding failed").expect("No message decoded");

        assert_eq!(message, decoded_message);
        assert!(buffer.is_empty(), "Buffer should be empty after decoding");
    }

    #[test]
    fn test_decode_not_enough_data_for_header() {
        let mut codec = DoipCodec {};
        let mut buffer = BytesMut::from(&[0x02, 0xfd, 0x00, 0x05][..]); // Only 4 bytes

        let result = codec.decode(&mut buffer).expect("Decoding should not fail");

        assert!(result.is_none());
        assert_eq!(buffer.len(), 4, "Buffer should not be consumed");
    }

    #[test]
    fn test_decode_header_present_but_incomplete_payload() {
        let mut codec = DoipCodec {};
        let message = create_routing_activation_request();
        let mut encoded_buffer = BytesMut::new();
        codec.encode(message, &mut encoded_buffer).unwrap();

        // Truncate buffer to be header + partial payload
        let partial_len = DOIP_HEADER_LENGTH + 2;
        encoded_buffer.truncate(partial_len);

        let result = codec.decode(&mut encoded_buffer).expect("Decoding should not fail");

        assert!(result.is_none());
        assert_eq!(encoded_buffer.len(), partial_len, "Buffer should not be consumed");
    }

    #[test]
    fn test_decode_with_extra_data() {
        let mut codec = DoipCodec {};
        let message = create_routing_activation_request();
        let mut buffer = BytesMut::new();
        codec.encode(message.clone(), &mut buffer).unwrap();

        // Add extra data
        let extra_data = &[0xDE, 0xAD, 0xBE, 0xEF];
        buffer.extend_from_slice(extra_data);

        let decoded_message =
            codec.decode(&mut buffer).expect("Decoding failed").expect("No message decoded");

        assert_eq!(message, decoded_message);
        assert_eq!(buffer.as_ref(), extra_data, "Buffer should contain only the extra data");
    }

    #[test]
    fn test_decode_invalid_header() {
        let mut codec = DoipCodec {};
        // Invalid protocol version
        let mut buffer = BytesMut::from(&[0xFD, 0xFD, 0x00, 0x05, 0x00, 0x00, 0x00, 0x07][..]);

        let result = codec.decode(&mut buffer);

        assert_that!(result, err(matches_pattern!(DoipCodecError::InvalidHeader(_))));
    }

    #[test]
    fn test_decode_invalid_payload() {
        let mut codec = DoipCodec {};
        // A RoutingActivationResponse requires a payload of 9 bytes. We provide 0.
        let header = DoipHeader {
            protocol_version: ProtocolVersion::Iso13400_2019,
            inverse_protocol_version: !(ProtocolVersion::Iso13400_2019 as u8),
            payload_type: PayloadType::RoutingActivationResponse,
            payload_length: 0,
        };
        let header_bytes: [u8; DOIP_HEADER_LENGTH] = header.into();
        let mut buffer = BytesMut::from(&header_bytes[..]);

        let result = codec.decode(&mut buffer);

        assert_that!(result, err(matches_pattern!(DoipCodecError::InvalidPayload(_))));
    }

    #[test]
    fn test_encode_routing_activation_response() {
        let mut codec = DoipCodec {};
        let payload = DoipPayload::RoutingActivationResponse(RoutingActivationResponse {
            logical_address: 0x0E00_u16.to_be_bytes(),
            source_address: 0x1000_u16.to_be_bytes(),
            activation_code: ActivationCode::SuccessfullyActivated,
            buffer: [0; 4],
        });
        let payload_bytes: Vec<u8> = payload.clone().into();
        let message = DoipMessage {
            header: DoipHeader {
                protocol_version: ProtocolVersion::Iso13400_2019,
                inverse_protocol_version: !(ProtocolVersion::Iso13400_2019 as u8),
                payload_type: PayloadType::RoutingActivationResponse,
                payload_length: payload_bytes.len() as u32,
            },
            payload,
        };

        let mut buffer = BytesMut::new();
        codec.encode(message, &mut buffer).expect("Encoding failed");

        assert_eq!(buffer.len(), DOIP_HEADER_LENGTH + 9);
    }
}
