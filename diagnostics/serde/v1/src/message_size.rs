// Copyright 2025 Google LLC

use crate::{enum_options::EnumOptions, field_options::FieldOptions, translator::TranslatorError};
use protobuf::reflect::{FieldDescriptor, MessageDescriptor, RuntimeFieldType::*, RuntimeType};
use vsidl_v1::diagnostics::ByteSize;

pub(crate) fn uds_message_size(descriptor: &MessageDescriptor) -> Result<usize, TranslatorError> {
    let mut size: usize = 0;

    for field_descriptor in descriptor.fields() {
        match field_descriptor.runtime_field_type() {
            Singular(runtime_type) => {
                size = size
                    .checked_add(field_size(&field_descriptor, &runtime_type)?)
                    .ok_or(TranslatorError::DiagnosticsMessageSizeCalculation)?;
            }
            Repeated(runtime_type) => {
                let repeated_field_count =
                    field_descriptor
                        .repeated_field_count()
                        .ok()
                        .flatten()
                        .ok_or(TranslatorError::DiagnosticsMessageSizeCalculation)?;

                let field_size = repeated_field_count
                    .checked_mul(field_size(&field_descriptor, &runtime_type)?)
                    .ok_or(TranslatorError::DiagnosticsMessageSizeCalculation)?;

                size = size
                    .checked_add(field_size)
                    .ok_or(TranslatorError::DiagnosticsMessageSizeCalculation)?;
            }
            Map(_, _) => unreachable!("map fields are not supported"),
        };
    }

    Ok(size)
}

fn field_size(
    descriptor: &FieldDescriptor,
    runtime_type: &RuntimeType,
) -> Result<usize, TranslatorError> {
    if descriptor.skip_for_diag() {
        return Ok(0);
    }

    match runtime_type {
        RuntimeType::I32 | RuntimeType::U32 | RuntimeType::F32 => {
            match descriptor.primitive_byte_size() {
                Some(ByteSize::ONE) => Ok(1),
                Some(ByteSize::TWO) => Ok(2),
                Some(ByteSize::THREE) => Ok(3),
                _ => Ok(4),
            }
        }
        RuntimeType::I64 | RuntimeType::U64 | RuntimeType::F64 => Ok(8),
        RuntimeType::Bool => Ok(1),
        RuntimeType::String | RuntimeType::VecU8 => descriptor
            .variable_type_size()
            .ok()
            .flatten()
            .ok_or(TranslatorError::DiagnosticsMessageSizeCalculation),
        RuntimeType::Enum(enum_descriptor) => match enum_descriptor.enum_byte_size() {
            Some(ByteSize::ONE) => Ok(1),
            Some(ByteSize::TWO) => Ok(2),
            Some(ByteSize::THREE) => Ok(3),
            _ => Ok(4),
        },
        RuntimeType::Message(message_descriptor) => uds_message_size(message_descriptor),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_message_descriptor::*;
    use crate::translator::TranslatorError;

    #[test]
    fn test_uds_message_size() {
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

        assert_eq!(uds_message_size(&message_descriptor).unwrap(), 83)
    }

    #[test]
    fn test_uds_message_size_overflow() {
        let message_descriptor = get_message_descriptor(
            r#"syntax = 'proto3';

            import "sdv/vsidl/v1/annotations.proto";
            import "sdv/diagnostics/v1/diagnostics.proto";

            message TestProto {
                repeated string my_string1 = 1 [(sdv.vsidl.v1.variable_type_max_size) = 2147483647, (sdv.vsidl.v1.repeated_field_max_count) = 2147483647];
                repeated string my_string2 = 2 [(sdv.vsidl.v1.variable_type_max_size) = 2147483647, (sdv.vsidl.v1.repeated_field_max_count) = 2147483647];
                repeated string my_string3 = 3 [(sdv.vsidl.v1.variable_type_max_size) = 2147483647, (sdv.vsidl.v1.repeated_field_max_count) = 2147483647];
                repeated string my_string4 = 4 [(sdv.vsidl.v1.variable_type_max_size) = 2147483647, (sdv.vsidl.v1.repeated_field_max_count) = 2147483647];
                repeated string my_string5 = 5 [(sdv.vsidl.v1.variable_type_max_size) = 2147483647, (sdv.vsidl.v1.repeated_field_max_count) = 2147483647];
            }
            "#,
            ".TestProto",
        );

        let message_size = uds_message_size(&message_descriptor);

        match message_size {
            Result::Err(TranslatorError::DiagnosticsMessageSizeCalculation) => {}
            _ => panic!("{message_size:#?}"),
        }
    }

    #[test]
    fn test_diagnostics_message_max_size_missing() {
        let message_descriptor = get_message_descriptor(
            r#"syntax = 'proto3';

            import "sdv/vsidl/v1/annotations.proto";
            import "sdv/diagnostics/v1/diagnostics.proto";

            message TestProto {
                repeated string my_string1 = 1 [(sdv.vsidl.v1.repeated_field_max_count) = 2147483647];
            }
            "#,
            ".TestProto",
        );

        let message_size = uds_message_size(&message_descriptor);

        match message_size {
            Result::Err(TranslatorError::DiagnosticsMessageSizeCalculation) => {}
            _ => panic!("{message_size:#?}"),
        }
    }

    #[test]
    fn test_diagnostics_message_max_count_missing() {
        let message_descriptor = get_message_descriptor(
            r#"syntax = 'proto3';

            import "sdv/vsidl/v1/annotations.proto";

            message TestProto {
                repeated string my_string1 = 1 [(sdv.vsidl.v1.variable_type_max_size) = 2147483647];
            }
            "#,
            ".TestProto",
        );

        let message_size = uds_message_size(&message_descriptor);

        match message_size {
            Result::Err(TranslatorError::DiagnosticsMessageSizeCalculation) => {}
            _ => panic!("{message_size:#?}"),
        }
    }
}
