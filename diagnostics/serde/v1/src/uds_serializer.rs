// Copyright 2024 Google LLC
use bytes::BufMut;
use serde::ser::{Impossible, SerializeStruct, SerializeTuple};
use serde::{Serialize, Serializer};

pub(crate) struct UdsSerializer<'buffer_lt> {
    output: &'buffer_lt mut dyn BufMut,
}

impl<'buffer_lt> UdsSerializer<'buffer_lt> {
    pub fn new(output: &'buffer_lt mut dyn BufMut) -> Self {
        UdsSerializer { output }
    }
}

impl<'ser_lt, 'buffer_lt> Serializer for &'ser_lt mut UdsSerializer<'buffer_lt> {
    type Ok = ();
    type Error = UdsSerializerError;
    type SerializeSeq = Impossible<(), UdsSerializerError>;
    type SerializeTuple = UdsTupleSerializer<'ser_lt, 'buffer_lt>;
    type SerializeTupleStruct = Impossible<(), UdsSerializerError>;
    type SerializeTupleVariant = Impossible<(), UdsSerializerError>;
    type SerializeMap = Impossible<(), UdsSerializerError>;
    type SerializeStruct = UdsTupleSerializer<'ser_lt, 'buffer_lt>;
    type SerializeStructVariant = Impossible<(), UdsSerializerError>;

    fn serialize_bool(self, value: bool) -> Result<Self::Ok, Self::Error> {
        if value {
            self.output.put_u8(0x01);
        } else {
            self.output.put_u8(0x00);
        }

        Ok(())
    }

    fn serialize_i8(self, value: i8) -> Result<Self::Ok, Self::Error> {
        self.output.put_i8(value);
        Ok(())
    }

    fn serialize_i16(self, value: i16) -> Result<Self::Ok, Self::Error> {
        self.output.put_i16(value);
        Ok(())
    }

    fn serialize_i32(self, value: i32) -> Result<Self::Ok, Self::Error> {
        self.output.put_i32(value);
        Ok(())
    }

    fn serialize_i64(self, value: i64) -> Result<Self::Ok, Self::Error> {
        self.output.put_i64(value);
        Ok(())
    }

    fn serialize_u8(self, value: u8) -> Result<Self::Ok, Self::Error> {
        self.output.put_u8(value);
        Ok(())
    }

    fn serialize_u16(self, value: u16) -> Result<Self::Ok, Self::Error> {
        self.output.put_u16(value);
        Ok(())
    }

    fn serialize_u32(self, value: u32) -> Result<Self::Ok, Self::Error> {
        self.output.put_u32(value);
        Ok(())
    }

    fn serialize_u64(self, value: u64) -> Result<Self::Ok, Self::Error> {
        self.output.put_u64(value);
        Ok(())
    }

    fn serialize_f32(self, value: f32) -> Result<Self::Ok, Self::Error> {
        self.output.put_f32(value);
        Ok(())
    }

    fn serialize_f64(self, value: f64) -> Result<Self::Ok, Self::Error> {
        self.output.put_f64(value);
        Ok(())
    }

    fn serialize_char(self, _value: char) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::UnsupportedType("Char (use u8 instead)".to_string()))
    }

    fn serialize_str(self, _: &str) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::UnsupportedType("Variable-size string".to_string()))
    }

    fn serialize_bytes(self, _: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::UnsupportedType("Variable-size byte array".to_string()))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::UnsupportedType("Empty optional value".to_string()))
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        unimplemented!()
    }

    fn serialize_unit_struct(self, _: &'static str) -> Result<Self::Ok, Self::Error> {
        unimplemented!()
    }

    fn serialize_unit_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::UnsupportedType("Unit variant".to_string()))
    }

    fn serialize_newtype_struct<T>(self, _: &'static str, _: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        unimplemented!()
    }

    fn serialize_newtype_variant<T>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        Err(Self::Error::UnsupportedType("Newtype variant".to_string()))
    }

    fn serialize_seq(
        self,
        _: std::option::Option<usize>,
    ) -> Result<Self::SerializeSeq, Self::Error> {
        Err(Self::Error::UnsupportedType("Variable-size sequence".to_string()))
    }

    fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(UdsTupleSerializer { serializer: self })
    }

    fn serialize_tuple_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        unimplemented!()
    }

    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(Self::Error::UnsupportedType("Tuple variant".to_string()))
    }

    fn serialize_map(
        self,
        _: std::option::Option<usize>,
    ) -> Result<Self::SerializeMap, Self::Error> {
        unimplemented!()
    }

    fn serialize_struct(
        self,
        _: &'static str,
        size: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.serialize_tuple(size)
    }

    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(Self::Error::UnsupportedType("Struct variant".to_string()))
    }
}

pub struct UdsTupleSerializer<'ser_lt, 'buffer_lt> {
    serializer: &'ser_lt mut UdsSerializer<'buffer_lt>,
}

impl SerializeTuple for UdsTupleSerializer<'_, '_> {
    type Ok = ();
    type Error = UdsSerializerError;

    fn serialize_element<T>(&mut self, element_value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        element_value.serialize(&mut *self.serializer)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl SerializeStruct for UdsTupleSerializer<'_, '_> {
    type Ok = ();
    type Error = UdsSerializerError;

    fn serialize_field<T>(&mut self, _: &'static str, field_value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        field_value.serialize(&mut *self.serializer)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UdsSerializerError {
    /// Custom serialization error
    #[error("Custom serialization error: {0}")]
    CustomError(String),
    /// Error serializing `char` as UTF-16
    #[error("Char is not a UTF-16 character: {0}")]
    CharEncodingError(char),
    /// UDS format doesn't support the type from `serde` type model.
    #[error("Serde type is not supported: {0}")]
    UnsupportedType(String),
}

impl serde::ser::Error for UdsSerializerError {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        UdsSerializerError::CustomError(msg.to_string())
    }
}

#[cfg(test)]
mod tests {
    use crate::uds_serializer::{UdsSerializer, UdsSerializerError};
    use bytes::{BufMut, BytesMut};
    use googletest::prelude::{assert_that, eq, matches_pattern};
    use serde::Serialize;

    #[test]
    fn serialize_structure_with_all_supported_fields() {
        #[derive(Serialize)]
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

        #[derive(Serialize)]
        struct InnerStruct {
            my_fixed_bytes: [u8; 3],
            my_fixed_string: [u8; 8],
        }

        let value = TestStruct {
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

        let mut expected_bytes = BytesMut::new();
        expected_bytes.put_i8(-8);
        expected_bytes.put_i16(-16);
        expected_bytes.put_i32(-32);
        expected_bytes.put_i64(-64);
        expected_bytes.put_u8(8);
        expected_bytes.put_u16(16);
        expected_bytes.put_u32(32);
        expected_bytes.put_u64(64);
        expected_bytes.put_f32(3.2);
        expected_bytes.put_f64(6.4);
        expected_bytes.put_u8(0x00);
        expected_bytes.put(&[1, 2, 3][..]);
        for byte in "hello❤".bytes() {
            expected_bytes.put_u8(byte);
        }

        let mut byte_buffer = BytesMut::new();
        let mut serializer = UdsSerializer::new(&mut byte_buffer);

        assert_that!(value.serialize(&mut serializer), matches_pattern!(Ok(())));
        assert_that!(byte_buffer.freeze(), eq(&expected_bytes.freeze()));
    }

    #[test]
    fn serialize_structure_variable_string_not_supported() {
        #[derive(Serialize)]
        struct TestStruct {
            my_string: String,
        }

        let value = TestStruct { my_string: "Hello".to_string() };

        let mut expected_bytes = BytesMut::new();
        for utf16_char in "Hello".encode_utf16() {
            expected_bytes.put_u16(utf16_char);
        }

        let mut byte_buffer = BytesMut::new();
        let mut serializer = UdsSerializer::new(&mut byte_buffer);

        assert_that!(
            value.serialize(&mut serializer),
            matches_pattern!(Err(matches_pattern!(UdsSerializerError::UnsupportedType(eq(
                "Variable-size string"
            )))))
        );
    }

    #[test]
    fn serialize_structure_variable_byte_array_not_supported() {
        #[derive(Serialize)]
        struct TestStruct {
            my_bytes: &'static [u8],
        }

        let value = TestStruct { my_bytes: &[1, 2, 3] };

        let mut expected_bytes = BytesMut::new();
        expected_bytes.put(&[1, 2, 3][..]);

        let mut byte_buffer = BytesMut::new();
        let mut serializer = UdsSerializer::new(&mut byte_buffer);

        assert_that!(
            value.serialize(&mut serializer),
            matches_pattern!(Err(matches_pattern!(UdsSerializerError::UnsupportedType(eq(
                "Variable-size sequence"
            )))))
        );
    }
}
