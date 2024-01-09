// Copyright 2024 Google LLC
use protobuf::reflect::FieldDescriptor;
use protobuf::UnknownValueRef;
use std::num::TryFromIntError;
use std::result::Result;

/// Trait that allows to retrieve protobuf field options that affect serialization /
/// deserialization process.
pub(crate) trait FieldOptions {
    fn repeated_field_count(&self) -> Result<Option<usize>, TryFromIntError>;
    fn variable_type_size(&self) -> Result<Option<usize>, TryFromIntError>;
}

/// Helper function for retrieving `usize` value of a field option by its id.
fn get_size_option(
    field_descriptor: &FieldDescriptor,
    option_id: u32,
) -> Result<Option<usize>, TryFromIntError> {
    if let Some(UnknownValueRef::Varint(value)) =
        field_descriptor.proto().options.special_fields.unknown_fields().get(option_id)
    {
        value.try_into().map(Some)
    } else {
        Ok(None)
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
}
