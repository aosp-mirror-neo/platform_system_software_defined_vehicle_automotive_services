// Copyright 2024 Google LLC
use protobuf::reflect::EnumDescriptor;
use protobuf::UnknownValueRef::{self, Varint};
use vsidl_syntax::diagnostics::ByteSize;

/// Trait that allows to retrieve protobuf field options that affect serialization /
/// deserialization process.
pub(crate) trait EnumOptions {
    fn enum_byte_size(&self) -> Option<ByteSize>;
}

/// Helper function for retrieving `u64` value of a field option by its id.
fn get_varint_option(enum_descriptor: &EnumDescriptor, option_id: u32) -> Option<u64> {
    get_option_by_id(enum_descriptor, option_id).and_then(|unknown_value| {
        if let Varint(value) = unknown_value {
            Some(value)
        } else {
            None
        }
    })
}

/// Helper function for retrieving option value as an `UnknownValueRef`.
fn get_option_by_id(enum_descriptor: &EnumDescriptor, option_id: u32) -> Option<UnknownValueRef> {
    enum_descriptor.proto().options.special_fields.unknown_fields().get(option_id)
}

impl EnumOptions for EnumDescriptor {
    fn enum_byte_size(&self) -> Option<ByteSize> {
        // id of sdv.diag.enum_type_size field option
        const ENUM_BYTE_SIZE: u32 = 50200;

        // convert raw int value to enum value
        match get_varint_option(self, ENUM_BYTE_SIZE) {
            Some(1) => Some(ByteSize::ONE),
            Some(2) => Some(ByteSize::TWO),
            _ => None,
        }
    }
}
