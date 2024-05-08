// Copyright 2024 Google LLC
use protobuf::reflect::FieldDescriptor;
use protobuf::Message;
use protobuf::UnknownValueRef::{self, *};
use sdv_vsidl_common::field_options::FieldOptions as VsidlFieldOptions;
use std::num::TryFromIntError;
use std::result::Result;
use vsidl_syntax::diagnostics::{ByteSize, FixedPointEncoding};

/// Trait that allows to retrieve protobuf field options that affect serialization /
/// deserialization process.
pub(crate) trait FieldOptions {
    fn repeated_field_count(&self) -> Result<Option<usize>, TryFromIntError>;
    fn variable_type_size(&self) -> Result<Option<usize>, TryFromIntError>;
    fn primitive_byte_size(&self) -> Option<ByteSize>;
    fn fixed_point_encoding(&self) -> Option<FixedPointEncoding>;

    // Convenience method for retrieving all options that affect `float` field
    // encoding.
    //
    // This method also enforces that (sdv.vsidl.primitive_byte_size) option is only
    // applicable when (sdv.vsidl.fixed_point_encoding) option is present.
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
            Some(0) => Some(ByteSize::ONE),
            Some(1) => Some(ByteSize::TWO),
            _ => None,
        }
    }

    fn fixed_point_encoding(&self) -> Option<FixedPointEncoding> {
        const FIXED_POINT_ENCODING: u32 = 50201;

        get_option_by_id(self, FIXED_POINT_ENCODING)
            .and_then(
                |unknown_value| {
                    if let LengthDelimited(value) = unknown_value {
                        Some(value)
                    } else {
                        None
                    }
                },
            )
            .map(FixedPointEncoding::parse_from_bytes)
            .and_then(Result::ok)
    }
}
