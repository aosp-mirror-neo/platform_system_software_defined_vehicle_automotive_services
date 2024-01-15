// Copyright 2024 Google LLC
use crate::protobuf_deserialize::MessageDeserializeSeed;
use crate::protobuf_serialize::MessageSerialize;
use crate::uds_deserializer::{UdsDeserializer, UdsDeserializerError};
use crate::uds_serializer::{UdsSerializer, UdsSerializerError};
use bytes::BytesMut;
use protobuf::reflect::{MessageDescriptor, MessageRef};
use serde::de::DeserializeSeed;
use serde::Serialize;

/// UDS to SDV translator.
/// Provides methods to translate protobuf bytes to UDS-compatible bytes to and back.
pub struct Translator;

impl Translator {
    /// Translates serialized protobuf message `proto_bytes` to UDS message.
    /// Method uses `message_descriptor` to perform reflection-based translation.
    /// Returns serialized UDS message or `TranslationError`.
    pub fn translate_from_protobuf_to_uds(
        &self,
        proto_bytes: &[u8],
        message_descriptor: &MessageDescriptor,
    ) -> Result<Vec<u8>, TranslationError> {
        let mut input_stream = protobuf::CodedInputStream::from_bytes(proto_bytes);

        let message = message_descriptor
            .parse_from(&mut input_stream)
            .map_err(TranslationError::ProtobufParsingError)?;

        let mut buffer = BytesMut::new();
        let mut serializer = UdsSerializer::new(&mut buffer);

        MessageSerialize::new(message_descriptor, &MessageRef::new(&*message))
            .serialize(&mut serializer)?;
        Ok(buffer.to_vec())
    }

    /// Translates serialized UDS message `proto_bytes` to protobuf message.
    /// Method uses `message_descriptor` to perform reflection-based translation.
    /// Returns serialized protobuf message or `TranslationError`.
    pub fn translate_from_uds_to_protobuf(
        &self,
        uds_bytes: &[u8],
        message_descriptor: &MessageDescriptor,
    ) -> Result<Vec<u8>, TranslationError> {
        let mut buffer = BytesMut::new();
        buffer.extend_from_slice(uds_bytes);
        let mut deserializer = UdsDeserializer::new(&mut buffer);

        MessageDeserializeSeed::new(message_descriptor)
            .deserialize(&mut deserializer)?
            .write_length_delimited_to_bytes_dyn()
            .map_err(TranslationError::ProtobufSerializationError)
    }
}

/// Error type for translation between protobuf and UDS formats
#[derive(Debug, thiserror::Error)]
pub enum TranslationError {
    /// Generic error during translation
    #[error("Generic error")]
    GenericError,
    /// Error when parsing message from proto format,
    #[error("Protobuf parsing error: {0}")]
    ProtobufParsingError(protobuf::Error),
    /// Error when serializing message to proto format,
    #[error("Protobuf serialization error: {0}")]
    ProtobufSerializationError(protobuf::Error),
    /// Error when deserializing message from UDS format,
    #[error("UDS parsing error: {0}")]
    UdsDeserializationError(#[from] UdsDeserializerError),
    /// Error when serializing message to UDS format,
    #[error("Protobuf serialization error: {0}")]
    UdsSerializationError(#[from] UdsSerializerError),
}

#[cfg(test)]
mod tests {
    use crate::translator::runtime_message_descriptor::get_message_descriptor;
    use crate::translator::Translator;

    #[test]
    #[should_panic]
    fn translate_from_protobuf_to_uds_not_implemented() {
        let translator = Translator {};

        let message_descriptor = get_message_descriptor(
            r#"syntax = 'proto3';

            message TestProto {
            }"#,
            ".TestProto",
        );

        let protobuf_bytes = vec![];

        let _ = translator.translate_from_protobuf_to_uds(&protobuf_bytes, &message_descriptor);
    }

    #[test]
    #[should_panic]
    fn translate_from_uds_to_protobuf_not_implemented() {
        let translator = Translator {};

        let message_descriptor = get_message_descriptor(
            r#"syntax = 'proto3';

            message TestProto {
            }"#,
            ".TestProto",
        );

        let uds_bytes = vec![];

        let _ = translator.translate_from_uds_to_protobuf(&uds_bytes, &message_descriptor);
    }
}

#[cfg(test)]
pub mod runtime_message_descriptor {
    use protobuf::reflect::{FileDescriptor, MessageDescriptor};
    use std::fs;
    use vsidl_syntax::vsidl;

    pub fn get_file_descriptor(proto_text: &str) -> FileDescriptor {
        let temp_dir = tempfile::tempdir().unwrap();

        let tempfile = temp_dir.path().join("test_proto.proto");
        fs::write(&tempfile, proto_text).unwrap();

        let file_descriptor_proto = protobuf_parse::Parser::new()
            .pure()
            .include(&temp_dir)
            // current folder includes VSIDL syntax definitions that might be required
            // to properly parse test proto file.
            .include(".")
            .input(&tempfile)
            .file_descriptor_set()
            .unwrap()
            .file[0]
            .clone();

        FileDescriptor::new_dynamic(
            file_descriptor_proto.clone(),
            &[vsidl::file_descriptor().clone()],
        )
        .unwrap()
    }

    pub fn get_message_descriptor(proto_text: &str, message_name: &str) -> MessageDescriptor {
        get_file_descriptor(proto_text).message_by_full_name(message_name).unwrap()
    }
}
