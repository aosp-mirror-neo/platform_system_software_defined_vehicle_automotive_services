// Copyright 2024 Google LLC

use byteorder::BigEndian;
use byteorder::ByteOrder;
use serde::de::{DeserializeSeed, SeqAccess, Visitor};
use serde::Deserializer;

pub(crate) struct UdsDeserializer<'buffer_lt> {
    input: &'buffer_lt [u8],
    index: usize,
}

impl<'buffer_lt> UdsDeserializer<'buffer_lt> {
    pub fn new(input: &'buffer_lt [u8]) -> Self {
        UdsDeserializer { input, index: 0 }
    }

    #[inline]
    fn check_index(&self, num_bytes: usize) -> Result<(), UdsDeserializerError> {
        if self.index + num_bytes > self.input.len() {
            Err(UdsDeserializerError::Incomplete)
        } else {
            Ok(())
        }
    }
}

impl<'buffer_lt> Deserializer<'buffer_lt> for &mut UdsDeserializer<'buffer_lt> {
    type Error = UdsDeserializerError;

    fn deserialize_any<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        Err(Self::Error::UnsupportedType("Any".to_string()))
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        self.check_index(1)?;
        let num = self.input[self.index];
        self.index += 1;
        let value = (num & 0x01) == 1;
        visitor.visit_bool(value)
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        self.check_index(1)?;
        let value = self.input[self.index] as i8;
        self.index += 1;
        visitor.visit_i8(value)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        self.check_index(2)?;
        let value = BigEndian::read_i16(&self.input[self.index..]);
        self.index += 2;
        visitor.visit_i16(value)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        self.check_index(4)?;
        let value = BigEndian::read_i32(&self.input[self.index..]);
        self.index += 4;
        visitor.visit_i32(value)
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        self.check_index(8)?;
        let value = BigEndian::read_i64(&self.input[self.index..]);
        self.index += 8;
        visitor.visit_i64(value)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        self.check_index(1)?;
        let value = self.input[self.index];
        self.index += 1;
        visitor.visit_u8(value)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        self.check_index(2)?;
        let value = BigEndian::read_u16(&self.input[self.index..]);
        self.index += 2;
        visitor.visit_u16(value)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        self.check_index(4)?;
        let value = BigEndian::read_u32(&self.input[self.index..]);
        self.index += 4;
        visitor.visit_u32(value)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        self.check_index(8)?;
        let value = BigEndian::read_u64(&self.input[self.index..]);
        self.index += 8;
        visitor.visit_u64(value)
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        self.check_index(4)?;
        let value = BigEndian::read_f32(&self.input[self.index..]);
        self.index += 4;
        visitor.visit_f32(value)
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        self.check_index(8)?;
        let value = BigEndian::read_f64(&self.input[self.index..]);
        self.index += 8;
        visitor.visit_f64(value)
    }

    fn deserialize_char<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        Err(Self::Error::UnsupportedType("Char (use u8 instead)".to_string()))
    }

    fn deserialize_str<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        Err(Self::Error::UnsupportedType("Variable-size string".to_string()))
    }

    fn deserialize_string<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        Err(Self::Error::UnsupportedType("Variable-size string".to_string()))
    }

    fn deserialize_bytes<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        Err(Self::Error::UnsupportedType("Variable-size byte array".to_string()))
    }

    fn deserialize_byte_buf<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        Err(Self::Error::UnsupportedType("Variable-size byte array".to_string()))
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        visitor.visit_some(self)
    }

    fn deserialize_unit<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        unimplemented!()
    }

    fn deserialize_unit_struct<V>(self, _: &'static str, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        unimplemented!()
    }

    fn deserialize_newtype_struct<V>(self, _: &'static str, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        unimplemented!()
    }

    fn deserialize_seq<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        Err(UdsDeserializerError::UnsupportedType("Variable-size sequence".to_string().to_string()))
    }

    fn deserialize_tuple<V>(self, size: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        let struct_deserializer = UdsTupleDeserializer::new(size, self);
        visitor.visit_seq(struct_deserializer)
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
        unimplemented!()
    }

    fn deserialize_map<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        unimplemented!()
    }

    fn deserialize_struct<V>(
        self,
        _: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        self.deserialize_tuple(fields.len(), visitor)
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
        Err(Self::Error::UnsupportedType("Enum".to_string()))
    }

    fn deserialize_identifier<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        Err(Self::Error::UnsupportedType("Identifier".to_string()))
    }

    fn deserialize_ignored_any<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'buffer_lt>,
    {
        Err(Self::Error::UnsupportedType("Ignored any".to_string()))
    }
}

struct UdsTupleDeserializer<'de_lt, 'buffer_lt> {
    size: usize,
    index: usize,
    deserializer: &'de_lt mut UdsDeserializer<'buffer_lt>,
}

impl<'de_lt, 'buffer_lt> UdsTupleDeserializer<'de_lt, 'buffer_lt> {
    fn new(size: usize, deserializer: &'de_lt mut UdsDeserializer<'buffer_lt>) -> Self {
        UdsTupleDeserializer { size, index: 0, deserializer }
    }
}

impl<'buffer_lt> SeqAccess<'buffer_lt> for UdsTupleDeserializer<'_, 'buffer_lt> {
    type Error = UdsDeserializerError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'buffer_lt>,
    {
        if self.index >= self.size {
            Ok(None)
        } else {
            self.index += 1;
            seed.deserialize(&mut *self.deserializer).map(Some)
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UdsDeserializerError {
    /// Custom deserialization error
    #[error("Custom deserialization error: {0}")]
    CustomError(String),
    /// UDS format doesn't support the type from `serde` type model.
    #[error("Serde type is not supported: {0}")]
    UnsupportedType(String),
    /// Incomplete data,
    #[error("Incomplete data")]
    Incomplete,
}

impl serde::de::Error for UdsDeserializerError {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        UdsDeserializerError::CustomError(msg.to_string())
    }
}

#[cfg(test)]
mod tests {
    use crate::uds_deserializer::{UdsDeserializer, UdsDeserializerError};
    use googletest::prelude::{assert_that, eq, matches_pattern};
    use serde::Deserialize;

    #[test]
    fn deserialize_structure_with_all_supported_fields() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct TestStruct {
            my_i8: i8,
            my_i16: i16,
            my_i32: i32,
            my_i64: i64,
            my_u8: u8,
            my_u16: u16,
            my_u32: u32,
            my_u64: u64,
            my_f32: f32,
            my_f64: f64,
            my_bool: bool,
            my_inner_struct: InnerStruct,
        }

        #[derive(Deserialize, Debug, PartialEq)]
        struct InnerStruct {
            my_fixed_bytes: [u8; 3],
            my_fixed_string: [u8; 8],
        }

        let expected_struct = TestStruct {
            my_i8: -8,
            my_i16: -16,
            my_i32: -32,
            my_i64: -64,
            my_u8: 8,
            my_u16: 16,
            my_u32: 32,
            my_u64: 64,
            my_f32: 3.2,
            my_f64: 6.4,
            my_bool: false,
            my_inner_struct: InnerStruct {
                my_fixed_bytes: [1, 2, 3],
                my_fixed_string: [b'h', b'e', b'l', b'l', b'o', b'\xE2', b'\x9D', b'\xA4'],
            },
        };

        let mut byte_buffer = Vec::<u8>::new();
        byte_buffer.push(-8i8 as u8);
        byte_buffer.extend((-16i16).to_be_bytes());
        byte_buffer.extend((-32i32).to_be_bytes());
        byte_buffer.extend((-64i64).to_be_bytes());
        byte_buffer.push(8);
        byte_buffer.extend(16u16.to_be_bytes());
        byte_buffer.extend(32u32.to_be_bytes());
        byte_buffer.extend(64u64.to_be_bytes());
        byte_buffer.extend(3.2f32.to_be_bytes());
        byte_buffer.extend(6.4f64.to_be_bytes());
        byte_buffer.push(0x00);
        byte_buffer.extend(&[1, 2, 3][..]);
        byte_buffer.extend("hello❤".as_bytes());

        let mut deserializer = UdsDeserializer::new(byte_buffer.as_ref());

        assert_that!(
            TestStruct::deserialize(&mut deserializer),
            matches_pattern!(Ok(eq(&expected_struct)))
        );
    }

    #[test]
    fn deserialize_structure_variable_string_not_supported() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct TestStruct {
            my_string: String,
        }

        let mut deserializer = UdsDeserializer::new("Hello".as_bytes());

        assert_that!(
            TestStruct::deserialize(&mut deserializer),
            matches_pattern!(Err(matches_pattern!(UdsDeserializerError::UnsupportedType(eq(
                "Variable-size string"
            )))))
        );
    }

    #[test]
    fn deserialize_structure_variable_byte_array_not_supported() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct TestStruct<'a> {
            my_bytes: &'a [u8],
        }
        let mut deserializer = UdsDeserializer::new(&[1, 2, 3]);
        assert_that!(
            TestStruct::deserialize(&mut deserializer),
            matches_pattern!(Err(matches_pattern!(UdsDeserializerError::UnsupportedType(eq(
                "Variable-size byte array"
            )))))
        );
    }
}
