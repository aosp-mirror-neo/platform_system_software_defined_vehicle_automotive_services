// Copyright 2024 Google LLC
use diag_serde_protos::diag_serde::PrimitiveByteSize;
use protobuf::reflect::FieldDescriptor;
use protobuf::UnknownValueRef;
use std::num::TryFromIntError;
use std::result::Result;

/// Trait that allows to retrieve protobuf field options that affect serialization /
/// deserialization process.
pub(crate) trait FieldOptions {
    fn repeated_field_count(&self) -> Result<Option<usize>, TryFromIntError>;
    fn variable_type_size(&self) -> Result<Option<usize>, TryFromIntError>;
    fn primitive_byte_size(&self) -> Option<PrimitiveByteSize>;
}

/// Helper function for retrieving `usize` value of a field option by its id.
fn get_size_option(
    field_descriptor: &FieldDescriptor,
    option_id: u32,
) -> Result<Option<usize>, TryFromIntError> {
    if let Some(value) = get_varint_option(field_descriptor, option_id) {
        value.try_into().map(Some)
    } else {
        Ok(None)
    }
}

/// Helper function for retrieving `u64` value of a field option by its id.
fn get_varint_option(field_descriptor: &FieldDescriptor, option_id: u32) -> Option<u64> {
    if let Some(UnknownValueRef::Varint(value)) =
        field_descriptor.proto().options.special_fields.unknown_fields().get(option_id)
    {
        Some(value)
    } else {
        None
    }
}

impl FieldOptions for FieldDescriptor {
    fn repeated_field_count(&self) -> Result<Option<usize>, TryFromIntError> {
        // id of sdv.vsidl.repeated_field_max_count field option
        const REPEATED_FIELD_COUNT: u32 = 50050;
        get_size_option(self, REPEATED_FIELD_COUNT)
    }

    fn variable_type_size(&self) -> Result<Option<usize>, TryFromIntError> {
        // id of sdv.vsidl.variable_type_max_size field option
        const VARIABLE_TYPE_SIZE: u32 = 50051;
        get_size_option(self, VARIABLE_TYPE_SIZE)
    }

    fn primitive_byte_size(&self) -> Option<PrimitiveByteSize> {
        // id of sdv.diag.primitive_byte_size field option
        const PRIMITIVE_BYTE_SIZE: u32 = 50200;

        // convert raw int value to enum value
        match get_varint_option(self, PRIMITIVE_BYTE_SIZE) {
            Some(0) => Some(PrimitiveByteSize::ONE),
            Some(1) => Some(PrimitiveByteSize::TWO),
            _ => None,
        }
    }
}
