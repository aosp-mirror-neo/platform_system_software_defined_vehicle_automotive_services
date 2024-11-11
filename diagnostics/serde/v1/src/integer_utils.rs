// Copyright 2024 Google LLC

use serde::de::Visitor;
use serde::ser::{Error, SerializeTuple};
use serde::{Deserialize, Deserializer};
use serde::{Serialize, Serializer};

pub(crate) fn serialize_i24<S: Serializer>(value: i32, serializer: S) -> Result<S::Ok, S::Error> {
    if !((i32::MIN >> 8)..=(i32::MAX >> 8)).contains(&value) {
        return Err(S::Error::custom("i24 overflow"));
    }

    let bytes = value.to_be_bytes();

    serialize_slice_as_array(&bytes[1..], serializer)
}

pub(crate) fn deserialize_i24<'de, D: Deserializer<'de>, V>(
    deserializer: D,
    visitor: V,
) -> Result<V::Value, D::Error>
where
    V: Visitor<'de>,
{
    let mut bytes = [0u8; 4];

    let deserialized_bytes = <[u8; 3]>::deserialize(deserializer)?;

    bytes[1..4].copy_from_slice(&deserialized_bytes[..]);

    // set the correct value of the first byte, depending on the sign bit
    if bytes[1] >= 0x80 {
        bytes[0] = 0xFF;
    }

    visitor.visit_i32(i32::from_be_bytes(bytes))
}

pub(crate) fn serialize_u24<S: Serializer>(value: u32, serializer: S) -> Result<S::Ok, S::Error> {
    if value > (u32::MAX >> 8) {
        return Err(S::Error::custom("u24 overflow"));
    }

    let bytes = value.to_be_bytes();

    serialize_slice_as_array(&bytes[1..], serializer)
}

pub(crate) fn deserialize_u24<'de, D: Deserializer<'de>, V>(
    deserializer: D,
    visitor: V,
) -> Result<V::Value, D::Error>
where
    V: Visitor<'de>,
{
    let mut bytes = [0u8; 4];

    let deserialized_bytes = <[u8; 3]>::deserialize(deserializer)?;

    bytes[1..4].copy_from_slice(&deserialized_bytes[..]);

    visitor.visit_u32(u32::from_be_bytes(bytes))
}

fn serialize_slice_as_array<T: Serialize, S: Serializer>(
    slice: &[T],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut tuple_serializer = serializer.serialize_tuple(slice.len())?;
    for element in slice {
        tuple_serializer.serialize_element(element)?;
    }
    tuple_serializer.end()
}
