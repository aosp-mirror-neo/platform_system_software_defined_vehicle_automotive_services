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
    ///
    /// `proto_bytes` are expected to be in format produced by
    /// `MessageDyn::write_to_bytes()`.
    pub fn translate_from_protobuf_to_uds(
        &self,
        proto_bytes: &[u8],
        message_descriptor: &MessageDescriptor,
    ) -> Result<Vec<u8>, TranslationError> {
        let message = message_descriptor
            .parse_from_bytes(proto_bytes)
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
    ///
    /// Returned proto bytes are in format produced by `MessageDyn::write_to_bytes()`.
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
            .write_to_bytes_dyn()
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
    use bytes::{BufMut, BytesMut};
    use std::vec::Vec;

    #[googletest::test]
    fn translate_from_protobuf_to_uds() {
        let translator = Translator {};

        let message_descriptor = get_message_descriptor(
            r#"syntax = 'proto3';

            import "syntax/vsidl.proto";
            import "syntax/diag_serde.proto";

            message TestProto {
                int64 my_i64 = 1;
                int32 my_i32 = 2;
                uint64 my_u64 = 3;
                uint32 my_u32 = 4;
                double my_f64 = 5;
                float my_f32 = 6;
                bool my_bool = 7;
                InnerProto my_inner_proto = 8;
                int32 my_one_byte_i32 = 9 [(sdv.diag.primitive_byte_size) = ONE];
                uint32 my_two_bytes_u32 = 10 [(sdv.diag.primitive_byte_size) = TWO];
                repeated uint32 my_one_byte_u32_array = 11 [
                    (sdv.diag.primitive_byte_size) = ONE,
                    (sdv.vsidl.repeated_field_max_count) = 2
                ];
            }

            message InnerProto {
                repeated bool my_fixed_array = 1 [(sdv.vsidl.repeated_field_max_count) = 5];
                string my_fixed_string = 2 [(sdv.vsidl.variable_type_max_size) = 10];
                TestEnum my_enum = 3;
            }

            enum TestEnum {
                A = 0;
                B = 1;
                C = 2;
            }
            "#,
            ".TestProto",
        );

        let mut proto_bytes: Vec<u8> = vec![];
        proto_bytes.put_u8(8); // VARINT field with field number '1'
        proto_bytes.put(&[192, 255, 255, 255, 255, 255, 255, 255, 255, 1][..]); // -64
        proto_bytes.put_u8(16); // VARINT field with field number '2'
        proto_bytes.put(&[224, 255, 255, 255, 255, 255, 255, 255, 255, 1][..]); // -32
        proto_bytes.put_u8(24); // VARINT field with field number '3'
        proto_bytes.put_u8(64); // +64
        proto_bytes.put_u8(32); // VARINT field with field number '4'
        proto_bytes.put_u8(32); // +32
        proto_bytes.put_u8(41); // I64 field with field number '5'
        proto_bytes.put(&[154, 153, 153, 153, 153, 153, 25, 64][..]); // 6.4_f64
        proto_bytes.put_u8(53); // I32 field with field number '6'
        proto_bytes.put(&[205, 204, 76, 192][..]);
        proto_bytes.put_u8(56); // VARINT field with field number '7'
        proto_bytes.put_u8(1); // false
        proto_bytes.put_u8(66); // LEN field with field number '8'
        proto_bytes.put_u8(24); // length of embedded message
        proto_bytes.put(&[8, 1, 8, 0, 8, 1, 8, 0, 8, 1][..]); // Repeated field with field number '1' and values
        proto_bytes.put_u8(18); // LEN field with field number '2'
        proto_bytes.put_u8(10); // String length == 10
        proto_bytes.put(&[104, 101, 108, 108, 111, 0, 0, 0, 0, 0][..]); // String "hello\0\0\0\0\0"
        proto_bytes.put_u8(24); // VARINT field with field number '3'
        proto_bytes.put_u8(1); // +1
        proto_bytes.put_u8(72); // VARINT field with field number '9'
        proto_bytes.put(&[136, 255, 255, 255, 255, 255, 255, 255, 255, 1][..]); // -120
        proto_bytes.put_u8(80); // VARINT field with field number '10'
        proto_bytes.put(&[128, 8][..]); // +1024
        proto_bytes.put(&[88, 10, 88, 20][..]); // Repeated VARINT field with field number '11'

        let mut expected_uds_bytes = BytesMut::new();
        expected_uds_bytes.put_i64(-64);
        expected_uds_bytes.put_i32(-32);
        expected_uds_bytes.put_u64(64);
        expected_uds_bytes.put_u32(32);
        expected_uds_bytes.put_f64(6.4);
        expected_uds_bytes.put_f32(-3.2);
        expected_uds_bytes.put_u8(1);
        expected_uds_bytes.put(&[1, 0, 1, 0, 1][..]);
        for char in "hello\0\0\0\0\0".chars() {
            expected_uds_bytes.put_u8(char.try_into().unwrap())
        }
        expected_uds_bytes.put_u32(1);
        expected_uds_bytes.put_i8(-120);
        expected_uds_bytes.put_u16(1024);
        expected_uds_bytes.put_u8(10);
        expected_uds_bytes.put_u8(20);
        let expected_uds_bytes = expected_uds_bytes.freeze();

        let uds_bytes =
            translator.translate_from_protobuf_to_uds(&proto_bytes, &message_descriptor).unwrap();
        assert_eq!(&uds_bytes, &expected_uds_bytes);
    }

    #[googletest::test]
    fn translate_from_uds_to_protobuf() {
        let translator = Translator {};

        let message_descriptor = get_message_descriptor(
            r#"syntax = 'proto3';

            import "syntax/vsidl.proto";
            import "syntax/diag_serde.proto";

            message TestProto {
                int64 my_i64 = 1;
                int32 my_i32 = 2;
                uint64 my_u64 = 3;
                uint32 my_u32 = 4;
                double my_f64 = 5;
                float my_f32 = 6;
                bool my_bool = 7;
                InnerProto my_inner_proto = 8;
                int32 my_one_byte_i32 = 9 [(sdv.diag.primitive_byte_size) = ONE];
                uint32 my_two_bytes_u32 = 10 [(sdv.diag.primitive_byte_size) = TWO];
                repeated uint32 my_one_byte_u32_array = 11 [
                    (sdv.diag.primitive_byte_size) = ONE,
                    (sdv.vsidl.repeated_field_max_count) = 2
                ];
            }

            message InnerProto {
                repeated bool my_fixed_array = 1 [(sdv.vsidl.repeated_field_max_count) = 5];
                string my_fixed_string = 2 [(sdv.vsidl.variable_type_max_size) = 10];
                TestEnum my_enum = 3;
            }

            enum TestEnum {
                A = 0;
                B = 1;
                C = 2;
            }
            "#,
            ".TestProto",
        );

        let mut uds_bytes = BytesMut::new();
        uds_bytes.put_i64(-64);
        uds_bytes.put_i32(-32);
        uds_bytes.put_u64(64);
        uds_bytes.put_u32(32);
        uds_bytes.put_f64(6.4);
        uds_bytes.put_f32(-3.2);
        uds_bytes.put_u8(1);
        uds_bytes.put(&[1, 0, 1, 0, 1][..]);
        for char in "hello\0\0\0\0\0".chars() {
            uds_bytes.put_u8(char.try_into().unwrap())
        }
        uds_bytes.put_u32(1);
        uds_bytes.put_i8(-120);
        uds_bytes.put_u16(1024);
        uds_bytes.put_u8(10);
        uds_bytes.put_u8(20);
        let uds_bytes = uds_bytes.freeze();

        let mut expected_proto_bytes: Vec<u8> = vec![];
        expected_proto_bytes.put_u8(8); // VARINT field with field number '1'
        expected_proto_bytes.put(&[192, 255, 255, 255, 255, 255, 255, 255, 255, 1][..]); // -64
        expected_proto_bytes.put_u8(16); // VARINT field with field number '2'
        expected_proto_bytes.put(&[224, 255, 255, 255, 255, 255, 255, 255, 255, 1][..]); // -32
        expected_proto_bytes.put_u8(24); // VARINT field with field number '3'
        expected_proto_bytes.put_u8(64); // +64
        expected_proto_bytes.put_u8(32); // VARINT field with field number '4'
        expected_proto_bytes.put_u8(32); // +32
        expected_proto_bytes.put_u8(41); // I64 field with field number '5'
        expected_proto_bytes.put(&[154, 153, 153, 153, 153, 153, 25, 64][..]); // 6.4_f64
        expected_proto_bytes.put_u8(53); // I32 field with field number '6'
        expected_proto_bytes.put(&[205, 204, 76, 192][..]);
        expected_proto_bytes.put_u8(56); // VARINT field with field number '7'
        expected_proto_bytes.put_u8(1); // false
        expected_proto_bytes.put_u8(66); // LEN field with field number '8'
        expected_proto_bytes.put_u8(24); // length of embedded message
        expected_proto_bytes.put(&[8, 1, 8, 0, 8, 1, 8, 0, 8, 1][..]); // Repeated field with field number '1' and values
        expected_proto_bytes.put_u8(18); // LEN field with field number '2'
        expected_proto_bytes.put_u8(10); // String length == 10
        expected_proto_bytes.put(&[104, 101, 108, 108, 111, 0, 0, 0, 0, 0][..]); // String "hello\0\0\0\0\0"
        expected_proto_bytes.put_u8(24); // VARINT field with field number '3'
        expected_proto_bytes.put_u8(1); // +1
        expected_proto_bytes.put_u8(72); // VARINT field with field number '9'
        expected_proto_bytes.put(&[136, 255, 255, 255, 255, 255, 255, 255, 255, 1][..]); // -120
        expected_proto_bytes.put_u8(80); // VARINT field with field number '10'
        expected_proto_bytes.put(&[128, 8][..]); // +1024
        expected_proto_bytes.put(&[88, 10, 88, 20][..]); // Repeated VARINT field with field number '11'

        let proto_bytes =
            translator.translate_from_uds_to_protobuf(&uds_bytes, &message_descriptor).unwrap();
        assert_eq!(&proto_bytes, &expected_proto_bytes);
    }
}

#[cfg(test)]
pub mod runtime_message_descriptor {
    use diag_serde_protos::diag_serde;
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
            &[vsidl::file_descriptor().clone(), diag_serde::file_descriptor().clone()],
        )
        .unwrap()
    }

    pub fn get_message_descriptor(proto_text: &str, message_name: &str) -> MessageDescriptor {
        get_file_descriptor(proto_text).message_by_full_name(message_name).unwrap()
    }
}
