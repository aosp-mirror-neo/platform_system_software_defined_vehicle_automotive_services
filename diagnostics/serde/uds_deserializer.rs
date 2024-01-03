// Copyright 2024 Google LLC
use bytes::BytesMut;
use serde::de::Visitor;
use serde::Deserializer;

pub(crate) struct UdsDeserializer<'buffer_lt> {
    _input: &'buffer_lt mut BytesMut,
}

impl<'buffer_lt> UdsDeserializer<'buffer_lt> {
    pub fn new(_input: &'buffer_lt mut BytesMut) -> Self {
        UdsDeserializer { _input }
    }
}

impl<'de_lt, 'buffer_lt> Deserializer<'buffer_lt> for &'de_lt mut UdsDeserializer<'buffer_lt> {
    type Error = UdsDeserializerError;

    fn deserialize_any<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        todo!()
    }

    fn deserialize_bool<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        todo!()
    }

    fn deserialize_i8<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        todo!()
    }

    fn deserialize_i16<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        todo!()
    }

    fn deserialize_i32<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        todo!()
    }

    fn deserialize_i64<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        todo!()
    }

    fn deserialize_u8<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        todo!()
    }

    fn deserialize_u16<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        todo!()
    }

    fn deserialize_u32<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        todo!()
    }

    fn deserialize_u64<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        todo!()
    }

    fn deserialize_f32<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        todo!()
    }

    fn deserialize_f64<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        todo!()
    }

    fn deserialize_char<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        todo!()
    }

    fn deserialize_str<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        todo!()
    }

    fn deserialize_string<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        todo!()
    }

    fn deserialize_bytes<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        todo!()
    }

    fn deserialize_byte_buf<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        todo!()
    }

    fn deserialize_option<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        todo!()
    }

    fn deserialize_unit<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        todo!()
    }

    fn deserialize_unit_struct<V>(self, _: &'static str, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        todo!()
    }

    fn deserialize_newtype_struct<V>(self, _: &'static str, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        todo!()
    }

    fn deserialize_seq<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        todo!()
    }

    fn deserialize_tuple<V>(self, _: usize, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        todo!()
    }

    fn deserialize_tuple_struct<V>(
        self,
        _: &'static str,
        _: usize,
        _: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        todo!()
    }

    fn deserialize_map<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        todo!()
    }

    fn deserialize_struct<V>(
        self,
        _: &'static str,
        _: &'static [&'static str],
        _: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        todo!()
    }

    fn deserialize_enum<V>(
        self,
        _: &'static str,
        _: &'static [&'static str],
        _: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        todo!()
    }

    fn deserialize_identifier<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        todo!()
    }

    fn deserialize_ignored_any<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        todo!()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UdsDeserializerError {
    // Custom deserialization error
    #[error("Custom deserialization error: {0}")]
    CustomError(String),
}

impl serde::de::Error for UdsDeserializerError {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        UdsDeserializerError::CustomError(msg.to_string())
    }
}
