// Copyright 2024 Google LLC
use crate::message_size::uds_message_size;
use crate::protobuf_deserialize::MessageDeserializeSeed;
use crate::protobuf_serialize::MessageSerialize;
use crate::uds_deserializer::{UdsDeserializer, UdsDeserializerError};
use crate::uds_serializer::{UdsSerializer, UdsSerializerError};
use mockall::automock;
use protobuf::reflect::{MessageDescriptor, MessageRef};
use serde::de::DeserializeSeed;
use serde::Serialize;
use std::sync::Arc;

/// UDS to SDV translator trait.
/// Provides methods to translate protobuf bytes to UDS-compatible bytes to and back,
/// taking protobuf as a description of message format.
#[automock]
pub trait Translator: Send + Sync {
    /// Translates serialized protobuf message `proto_bytes` to UDS message.
    /// Method uses `message_descriptor` to perform reflection-based translation.
    /// Returns serialized UDS message or `TranslatorError`.
    ///
    /// `proto_bytes` are expected to be in format produced by
    /// `MessageDyn::write_to_bytes()`.
    fn translate_from_protobuf_to_uds(
        &self,
        proto_bytes: &[u8],
        message_descriptor: &MessageDescriptor,
    ) -> Result<Vec<u8>, TranslatorError>;

    /// Translates serialized UDS message `proto_bytes` to protobuf message.
    /// Method uses `message_descriptor` to perform reflection-based translation.
    /// Returns serialized protobuf message or `TranslatorError`.
    ///
    /// Returned proto bytes are in format produced by `MessageDyn::write_to_bytes()`.
    fn translate_from_uds_to_protobuf(
        &self,
        uds_bytes: &[u8],
        message_descriptor: &MessageDescriptor,
    ) -> Result<Vec<u8>, TranslatorError>;
}

/// Create new UDS to SDV translator implementation.
pub fn new() -> Arc<dyn Translator> {
    Arc::new(TranslatorImpl)
}

/// UDS to SDV translator implementation.
struct TranslatorImpl;

impl Translator for TranslatorImpl {
    fn translate_from_protobuf_to_uds(
        &self,
        proto_bytes: &[u8],
        message_descriptor: &MessageDescriptor,
    ) -> Result<Vec<u8>, TranslatorError> {
        let message = message_descriptor
            .parse_from_bytes(proto_bytes)
            .map_err(TranslatorError::ProtobufParsing)?;

        // Calculate size of and try to allocate buffer
        let size = uds_message_size(message_descriptor)?;
        let mut buffer = Vec::<u8>::new();
        buffer.try_reserve(size).map_err(|_| TranslatorError::OutOfMemory)?;

        let mut serializer = UdsSerializer::new(&mut buffer);

        MessageSerialize::new(message_descriptor, &MessageRef::new(&*message))
            .serialize(&mut serializer)?;

        // Sanity check
        if buffer.len() != size {
            return Err(TranslatorError::Generic);
        }

        Ok(buffer)
    }

    fn translate_from_uds_to_protobuf(
        &self,
        uds_bytes: &[u8],
        message_descriptor: &MessageDescriptor,
    ) -> Result<Vec<u8>, TranslatorError> {
        let mut deserializer = UdsDeserializer::new(uds_bytes);

        MessageDeserializeSeed::new(message_descriptor)
            .deserialize(&mut deserializer)?
            .write_to_bytes_dyn()
            .map_err(TranslatorError::ProtobufSerialization)
    }
}

/// Error type for translation between protobuf and UDS formats
#[derive(Debug, thiserror::Error)]
pub enum TranslatorError {
    /// Internal error indicating an issue with translator implementation.
    #[error("Internal error indicating an issue with translator implementation")]
    Generic,
    /// Error when parsing message from proto format,
    #[error("Protobuf parsing error: {0}")]
    ProtobufParsing(protobuf::Error),
    /// Error when serializing message to proto format,
    #[error("Protobuf serialization error: {0}")]
    ProtobufSerialization(protobuf::Error),
    /// Error when deserializing message from UDS format,
    #[error("UDS parsing error: {0}")]
    UdsDeserialization(#[from] UdsDeserializerError),
    /// Error when serializing message to UDS format,
    #[error("Protobuf serialization error: {0}")]
    UdsSerialization(#[from] UdsSerializerError),
    /// Not enough memory to allocate a buffer.
    #[error("Error allocating buffer for translation result")]
    OutOfMemory,
    /// Error calculating size of message in diagnostics format.
    #[error("Diagnostics message size can not be calculated")]
    DiagnosticsMessageSizeCalculation,
}

#[cfg(test)]
mod tests {
    use crate::translator::runtime_message_descriptor::get_message_descriptor;
    use crate::translator::{Translator, TranslatorError, TranslatorImpl};
    use std::vec::Vec;

    #[googletest::test]
    fn translate_from_protobuf_to_uds() {
        let translator = TranslatorImpl {};

        let message_descriptor = get_message_descriptor(
            r#"syntax = 'proto3';

            import "sdv/vsidl/v1/annotations.proto";
            import "sdv/diagnostics/v1/diagnostics.proto";

            message TestProto {
                int64 my_i64 = 1;
                int32 my_i32 = 2;
                uint64 my_u64 = 3;
                uint32 my_u32 = 4;
                double my_f64 = 5;
                float my_f32 = 6;
                bool my_bool = 7;
                InnerProto my_inner_proto = 8;
                int32 my_one_byte_i32 = 9 [(sdv.diagnostics.v1.primitive_byte_size) = ONE];
                uint32 my_two_bytes_u32 = 10 [(sdv.diagnostics.v1.primitive_byte_size) = TWO];
                repeated uint32 my_one_byte_u32_array = 11 [
                    (sdv.diagnostics.v1.primitive_byte_size) = ONE,
                    (sdv.vsidl.v1.repeated_field_max_count) = 2
                ];
                float my_fixed_f32 = 12 [(sdv.diagnostics.v1.fixed_point_encoding) = {
                    scale: 0.001
                    offset: 20
                }];
                double my_fixed_f64 = 13 [(sdv.diagnostics.v1.fixed_point_encoding) = {
                    scale: 0.0001
                    offset: 0
                }];
                OneByteEnum my_one_byte_enum = 14;
                float my_one_byte_fixed_float = 15 [(sdv.diagnostics.v1.fixed_point_encoding) = {
                    scale: 0.001
                    offset: 20
                }, (sdv.diagnostics.v1.primitive_byte_size) = ONE];
                float my_two_byte_fixed_float = 16 [(sdv.diagnostics.v1.fixed_point_encoding) = {
                    scale: 0.001
                    offset: 20
                }, (sdv.diagnostics.v1.primitive_byte_size) = TWO];
                int32 my_skipped_i32 = 17 [(sdv.diagnostics.v1.skip_for_diag) = true];
                int32 my_three_byte_i32 = 18 [(sdv.diagnostics.v1.primitive_byte_size) = THREE];
                uint32 my_three_bytes_u32 = 19 [(sdv.diagnostics.v1.primitive_byte_size) = THREE];
            }

            message InnerProto {
                repeated bool my_fixed_array = 1 [(sdv.vsidl.v1.repeated_field_max_count) = 5];
                string my_fixed_string = 2 [(sdv.vsidl.v1.variable_type_max_size) = 10];
                TestEnum my_enum = 3;
            }

            enum OneByteEnum {
                option (sdv.diagnostics.v1.enum_byte_size) = ONE;

                A = 0;
                B = 1;
                C = 2;
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
        proto_bytes.push(8); // VARINT field with field number '1'
        proto_bytes.extend(&[192, 255, 255, 255, 255, 255, 255, 255, 255, 1][..]); // -64
        proto_bytes.push(16); // VARINT field with field number '2'
        proto_bytes.extend(&[224, 255, 255, 255, 255, 255, 255, 255, 255, 1][..]); // -32
        proto_bytes.push(24); // VARINT field with field number '3'
        proto_bytes.push(64); // +64
        proto_bytes.push(32); // VARINT field with field number '4'
        proto_bytes.push(32); // +32
        proto_bytes.push(41); // I64 field with field number '5'
        proto_bytes.extend(&6.4f64.to_le_bytes()[..]); // 6.4_f64
        proto_bytes.push(53); // I32 field with field number '6'
        proto_bytes.extend(&(-3.2f32).to_le_bytes()[..]);
        proto_bytes.push(56); // VARINT field with field number '7'
        proto_bytes.push(1); // false
        proto_bytes.push(66); // LEN field with field number '8'
        proto_bytes.push(24); // length of embedded message
        proto_bytes.extend(&[8, 1, 8, 0, 8, 1, 8, 0, 8, 1][..]); // Repeated field with field number '1' and values
        proto_bytes.push(18); // LEN field with field number '2'
        proto_bytes.push(10); // String length == 10
        proto_bytes.extend(&[104, 101, 108, 108, 111, 0, 0, 0, 0, 0][..]); // String "hello\0\0\0\0\0"
        proto_bytes.push(24); // VARINT field with field number '3'
        proto_bytes.push(1); // +1
        proto_bytes.push(72); // VARINT field with field number '9'
        proto_bytes.extend(&[136, 255, 255, 255, 255, 255, 255, 255, 255, 1][..]); // -120
        proto_bytes.push(80); // VARINT field with field number '10'
        proto_bytes.extend(&[128, 8][..]); // +1024
        proto_bytes.extend(&[88, 10, 88, 20][..]); // Repeated VARINT field with field number '11'
        proto_bytes.push(101); // I32 field with field number '12'
        proto_bytes.extend(&20.101f32.to_le_bytes()[..]);
        proto_bytes.push(105); // I64 field with field number '13'
        proto_bytes.extend(&(-0.010100000000000001f64).to_le_bytes()[..]);
        proto_bytes.push(112); // VARINT field with field number '14'
        proto_bytes.push(1); // 1 == OneByteEnum::B
        proto_bytes.push(125); // I32 field with field number '15'
        proto_bytes.extend(&20.101f32.to_le_bytes()[..]);
        proto_bytes.extend(&[133, 1][..]); // I32 field with field number '16'
        proto_bytes.extend(&20.101f32.to_le_bytes()[..]);
        proto_bytes.extend(&[136, 1][..]); // VARINT field with field number '17'
        proto_bytes.extend(&[128, 8][..]); // +1024
        proto_bytes.extend(&[144, 1][..]); // VARINT field with field number '18'
        proto_bytes.push(32); // +32
        proto_bytes.extend(&[152, 1][..]); // VARINT field with field number '19'
        proto_bytes.push(32); // +32

        let mut expected_uds_bytes = Vec::<u8>::new();
        expected_uds_bytes.extend((-64i64).to_be_bytes());
        expected_uds_bytes.extend((-32i32).to_be_bytes());
        expected_uds_bytes.extend(64u64.to_be_bytes());
        expected_uds_bytes.extend(32u32.to_be_bytes());
        expected_uds_bytes.extend(6.4f64.to_be_bytes());
        expected_uds_bytes.extend((-3.2f32).to_be_bytes());
        expected_uds_bytes.push(1);
        expected_uds_bytes.extend(&[1, 0, 1, 0, 1][..]);
        expected_uds_bytes.extend("hello\0\0\0\0\0".as_bytes());
        expected_uds_bytes.extend(1u32.to_be_bytes());
        expected_uds_bytes.push(-120i8 as u8);
        expected_uds_bytes.extend(1024u16.to_be_bytes());
        expected_uds_bytes.push(10);
        expected_uds_bytes.push(20);
        expected_uds_bytes.extend(101i32.to_be_bytes());
        expected_uds_bytes.extend((-101i64).to_be_bytes());
        expected_uds_bytes.push(1i8 as u8);
        expected_uds_bytes.push(101i8 as u8);
        expected_uds_bytes.extend(101i16.to_be_bytes());
        expected_uds_bytes.extend(&[0, 0, 32][..]);
        expected_uds_bytes.extend(&[0, 0, 32][..]);

        let uds_bytes =
            translator.translate_from_protobuf_to_uds(&proto_bytes, &message_descriptor).unwrap();
        assert_eq!(&uds_bytes, &expected_uds_bytes);
    }

    #[test]
    fn translate_from_protobuf_to_uds_with_out_of_memory_error() {
        let translator = TranslatorImpl {};

        let message_descriptor = get_message_descriptor(
            r#"syntax = 'proto3';

            import "sdv/vsidl/v1/annotations.proto";
            import "sdv/diagnostics/v1/diagnostics.proto";

            message TestProto {
                repeated string my_string1 = 1 [(sdv.vsidl.v1.variable_type_max_size) = 2147483647, (sdv.vsidl.v1.repeated_field_max_count) = 2147483647];
            }
            "#,
            ".TestProto",
        );

        let uds_bytes = translator.translate_from_protobuf_to_uds(&[], &message_descriptor);

        match uds_bytes {
            Result::Err(TranslatorError::OutOfMemory) => {}
            _ => panic!("{uds_bytes:#?}"),
        }
    }

    #[googletest::test]
    fn translate_from_uds_to_protobuf() {
        let translator = TranslatorImpl {};

        let message_descriptor = get_message_descriptor(
            r#"syntax = 'proto3';

            import "sdv/vsidl/v1/annotations.proto";
            import "sdv/diagnostics/v1/diagnostics.proto";

            message TestProto {
                int64 my_i64 = 1;
                int32 my_i32 = 2;
                uint64 my_u64 = 3;
                uint32 my_u32 = 4;
                double my_f64 = 5;
                float my_f32 = 6;
                bool my_bool = 7;
                InnerProto my_inner_proto = 8;
                int32 my_one_byte_i32 = 9 [(sdv.diagnostics.v1.primitive_byte_size) = ONE];
                uint32 my_two_bytes_u32 = 10 [(sdv.diagnostics.v1.primitive_byte_size) = TWO];
                repeated uint32 my_one_byte_u32_array = 11 [
                    (sdv.diagnostics.v1.primitive_byte_size) = ONE,
                    (sdv.vsidl.v1.repeated_field_max_count) = 2
                ];
                float my_fixed_f32 = 12 [(sdv.diagnostics.v1.fixed_point_encoding) = {
                    scale: 0.001
                    offset: 20
                }];
                double my_fixed_f64 = 13 [(sdv.diagnostics.v1.fixed_point_encoding) = {
                    scale: 0.0001
                    offset: 0
                }];
                OneByteEnum my_one_byte_enum = 14;
                float my_one_byte_fixed_float = 15 [(sdv.diagnostics.v1.fixed_point_encoding) = {
                    scale: 0.001
                    offset: 20
                }, (sdv.diagnostics.v1.primitive_byte_size) = ONE];
                float my_two_byte_fixed_float = 16 [(sdv.diagnostics.v1.fixed_point_encoding) = {
                    scale: 0.001
                    offset: 20
                }, (sdv.diagnostics.v1.primitive_byte_size) = TWO];
                int32 my_skipped_i32 = 17 [(sdv.diagnostics.v1.skip_for_diag) = true];
                int32 my_three_byte_i32 = 18 [(sdv.diagnostics.v1.primitive_byte_size) = THREE];
                uint32 my_three_bytes_u32 = 19 [(sdv.diagnostics.v1.primitive_byte_size) = THREE];
            }

            message InnerProto {
                repeated bool my_fixed_array = 1 [(sdv.vsidl.v1.repeated_field_max_count) = 5];
                string my_fixed_string = 2 [(sdv.vsidl.v1.variable_type_max_size) = 10];
                TestEnum my_enum = 3;
            }

            enum OneByteEnum {
                option (sdv.diagnostics.v1.enum_byte_size) = ONE;

                A = 0;
                B = 1;
                C = 2;
            }

            enum TestEnum {
                A = 0;
                B = 1;
                C = 2;
            }
            "#,
            ".TestProto",
        );

        let mut uds_bytes = Vec::<u8>::new();
        uds_bytes.extend((-64i64).to_be_bytes());
        uds_bytes.extend((-32i32).to_be_bytes());
        uds_bytes.extend(64u64.to_be_bytes());
        uds_bytes.extend(32u32.to_be_bytes());
        uds_bytes.extend(6.4f64.to_be_bytes());
        uds_bytes.extend((-3.2f32).to_be_bytes());
        uds_bytes.push(1u8);
        uds_bytes.extend(&[1, 0, 1, 0, 1][..]);
        uds_bytes.extend("hello\0\0\0\0\0".as_bytes());
        uds_bytes.extend(1u32.to_be_bytes());
        uds_bytes.push(-120i8 as u8);
        uds_bytes.extend(1024u16.to_be_bytes());
        uds_bytes.push(10u8);
        uds_bytes.push(20u8);
        uds_bytes.extend(101i32.to_be_bytes());
        uds_bytes.extend((-101i64).to_be_bytes());
        uds_bytes.push(1i8 as u8);
        uds_bytes.push(101i8 as u8);
        uds_bytes.extend(101i16.to_be_bytes());
        uds_bytes.extend(&[0, 0, 32][..]);
        uds_bytes.extend(&[0, 0, 32][..]);

        let mut expected_proto_bytes: Vec<u8> = vec![];
        expected_proto_bytes.push(8u8); // VARINT field with field number '1'
        expected_proto_bytes.extend(&[192, 255, 255, 255, 255, 255, 255, 255, 255, 1][..]); // -64
        expected_proto_bytes.push(16u8); // VARINT field with field number '2'
        expected_proto_bytes.extend(&[224, 255, 255, 255, 255, 255, 255, 255, 255, 1][..]); // -32
        expected_proto_bytes.push(24u8); // VARINT field with field number '3'
        expected_proto_bytes.push(64u8); // +64
        expected_proto_bytes.push(32u8); // VARINT field with field number '4'
        expected_proto_bytes.push(32u8); // +32
        expected_proto_bytes.push(41u8); // I64 field with field number '5'
        expected_proto_bytes.extend(&6.4f64.to_le_bytes()[..]); // 6.4_f64
        expected_proto_bytes.push(53u8); // I32 field with field number '6'
        expected_proto_bytes.extend(&(-3.2f32).to_le_bytes()[..]);
        expected_proto_bytes.push(56u8); // VARINT field with field number '7'
        expected_proto_bytes.push(1u8); // false
        expected_proto_bytes.push(66u8); // LEN field with field number '8'
        expected_proto_bytes.push(24u8); // length of embedded message
        expected_proto_bytes.extend(&[8, 1, 8, 0, 8, 1, 8, 0, 8, 1][..]); // Repeated field with field number '1' and values
        expected_proto_bytes.push(18u8); // LEN field with field number '2'
        expected_proto_bytes.push(10u8); // String length == 10
        expected_proto_bytes.extend(&[104, 101, 108, 108, 111, 0, 0, 0, 0, 0][..]); // String "hello\0\0\0\0\0"
        expected_proto_bytes.push(24u8); // VARINT field with field number '3'
        expected_proto_bytes.push(1u8); // +1
        expected_proto_bytes.push(72u8); // VARINT field with field number '9'
        expected_proto_bytes.extend(&[136, 255, 255, 255, 255, 255, 255, 255, 255, 1][..]); // -120
        expected_proto_bytes.push(80u8); // VARINT field with field number '10'
        expected_proto_bytes.extend(&[128, 8][..]); // +1024
        expected_proto_bytes.extend(&[88, 10, 88, 20][..]); // Repeated VARINT field with field number '11'
        expected_proto_bytes.push(101u8); // I32 field with field number '12'
        expected_proto_bytes.extend(&20.101f32.to_le_bytes()[..]);
        expected_proto_bytes.push(105u8); // I64 field with field number '13'
        expected_proto_bytes.extend(&(-0.010100000000000001f64).to_le_bytes()[..]);
        expected_proto_bytes.push(112u8); // VARINT field with field number '14'
        expected_proto_bytes.push(1u8); // 1 == OneByteEnum::B
        expected_proto_bytes.push(125u8); // I32 field with field number '15'
        expected_proto_bytes.extend(&20.101f32.to_le_bytes()[..]);
        expected_proto_bytes.extend(&[133, 1][..]); // I32 field with field number '16'
        expected_proto_bytes.extend(&20.101f32.to_le_bytes()[..]);
        expected_proto_bytes.extend(&[144, 1][..]); // VARINT field with field number '18'
        expected_proto_bytes.push(32u8); // +32
        expected_proto_bytes.extend(&[152, 1][..]); // VARINT field with field number '19'
        expected_proto_bytes.push(32u8); // +32

        let proto_bytes =
            translator.translate_from_uds_to_protobuf(&uds_bytes, &message_descriptor).unwrap();
        assert_eq!(&proto_bytes, &expected_proto_bytes);
    }
}

#[cfg(test)]
pub mod runtime_message_descriptor {
    use protobuf::reflect::{FileDescriptor, MessageDescriptor};
    use std::fs;
    use vsidl_v1::{annotations, diagnostics};

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
            &[annotations::file_descriptor().clone(), diagnostics::file_descriptor().clone()],
        )
        .unwrap()
    }

    pub fn get_message_descriptor(proto_text: &str, message_name: &str) -> MessageDescriptor {
        get_file_descriptor(proto_text).message_by_full_name(message_name).unwrap()
    }
}
