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
use protobuf::reflect::FieldDescriptor;
use protobuf::Message;
use protobuf::UnknownValueRef::{self, *};
use sdv_vsidl_common::field_options::FieldOptions as VsidlFieldOptions;
use std::num::TryFromIntError;
use std::result::Result;
use vsidl_v1::diagnostics::{ByteSize, FixedPointEncoding};

/// Trait that allows to retrieve protobuf field options that affect serialization /
/// deserialization process.
pub(crate) trait FieldOptions {
    fn repeated_field_count(&self) -> Result<Option<usize>, TryFromIntError>;
    fn variable_type_size(&self) -> Result<Option<usize>, TryFromIntError>;
    fn primitive_byte_size(&self) -> Option<ByteSize>;
    fn fixed_point_encoding(&self) -> Option<FixedPointEncoding>;
    fn skip_for_diag(&self) -> bool;

    // Convenience method for retrieving all options that affect `float` field
    // encoding.
    //
    // This method also enforces that (sdv.diagnostics.v1.primitive_byte_size) option is only
    // applicable when (sdv.diagnostics.v1.fixed_point_encoding) option is present.
    fn float_encoding(&self) -> Option<(FixedPointEncoding, Option<ByteSize>)> {
        self.fixed_point_encoding()
            .map(|fixed_point_encoding| (fixed_point_encoding, self.primitive_byte_size()))
    }
}

/// Helper function for retrieving `u64` value of a field option by its id.
fn get_varint_option(field_descriptor: &FieldDescriptor, option_id: u32) -> Option<u64> {
    get_option_by_id(field_descriptor, option_id).and_then(|unknown_value| {
        if let Varint(value) = unknown_value {
            Some(value)
        } else {
            None
        }
    })
}

fn get_length_delimited_option(
    field_descriptor: &FieldDescriptor,
    option_id: u32,
) -> Option<&[u8]> {
    get_option_by_id(field_descriptor, option_id).and_then(|unknown_value| {
        if let LengthDelimited(value) = unknown_value {
            Some(value)
        } else {
            None
        }
    })
}

/// Helper function for retrieving option value as an `UnknownValueRef`.
fn get_option_by_id(field_descriptor: &FieldDescriptor, option_id: u32) -> Option<UnknownValueRef> {
    field_descriptor.proto().options.special_fields.unknown_fields().get(option_id)
}

impl FieldOptions for FieldDescriptor {
    fn repeated_field_count(&self) -> Result<Option<usize>, TryFromIntError> {
        VsidlFieldOptions::repeated_field_max_count(self)
    }

    fn variable_type_size(&self) -> Result<Option<usize>, TryFromIntError> {
        VsidlFieldOptions::variable_type_max_size(self)
    }

    fn primitive_byte_size(&self) -> Option<ByteSize> {
        // id of sdv.vsidl.primitive_byte_size field option
        const PRIMITIVE_BYTE_SIZE: u32 = 50200;

        // convert raw int value to enum value
        match get_varint_option(self, PRIMITIVE_BYTE_SIZE) {
            Some(1) => Some(ByteSize::ONE),
            Some(2) => Some(ByteSize::TWO),
            Some(3) => Some(ByteSize::THREE),
            _ => None,
        }
    }

    fn fixed_point_encoding(&self) -> Option<FixedPointEncoding> {
        const FIXED_POINT_ENCODING: u32 = 50201;

        get_length_delimited_option(self, FIXED_POINT_ENCODING)
            .map(FixedPointEncoding::parse_from_bytes)
            .and_then(Result::ok)
    }

    fn skip_for_diag(&self) -> bool {
        const SKIP_FOR_DIAG: u32 = 50202;

        get_varint_option(self, SKIP_FOR_DIAG).map(|value| value != 0).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_message_descriptor::get_message_descriptor;

    #[test]
    fn test_mistyped_option() {
        let descriptor = get_message_descriptor(
            r#"syntax = 'proto3';

            import "sdv/diagnostics/v1/diagnostics.proto";

            message TestProto {
                float field_with_mistyped_options = 1 [
                    (sdv.diagnostics.v1.fixed_point_encoding) = {
                        scale: 0.001
                        offset: 20
                    },
                    (sdv.diagnostics.v1.primitive_byte_size) = ONE,
                    (sdv.diagnostics.v1.skip_for_diag) = true
                ];
            }
            "#,
            ".TestProto",
        );

        let field_descriptor = descriptor.field_by_name("field_with_mistyped_options").unwrap();

        // Options are present
        assert!(field_descriptor.fixed_point_encoding().is_some());
        assert!(field_descriptor.skip_for_diag());

        // Getting them with wrong type returns `None`
        assert!(get_varint_option(&field_descriptor, 50201).is_none());
        assert!(get_length_delimited_option(&field_descriptor, 50202).is_none());
    }
}
