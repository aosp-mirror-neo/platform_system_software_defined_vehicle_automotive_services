// Copyright 2024 Google LLC
use crate::enum_options::EnumOptions;
use crate::field_options::FieldOptions;
use diag_serde_protos::diag_serde::{ByteSize, FixedPointEncoding};
use protobuf::reflect::{
    FieldDescriptor, MessageDescriptor, MessageRef,
    ReflectFieldRef::{self, *},
    ReflectRepeatedRef,
    ReflectValueRef::{self, *},
};
use serde::ser::{Error, SerializeSeq, SerializeTuple};
use serde::{Serialize, Serializer};
use std::ops::Deref;

pub(crate) struct MessageSerialize<'lt> {
    descriptor: &'lt MessageDescriptor,
    message: &'lt MessageRef<'lt>,
}

impl<'lt> MessageSerialize<'lt> {
    pub fn new(descriptor: &'lt MessageDescriptor, message: &'lt MessageRef<'lt>) -> Self {
        Self { descriptor, message }
    }
}

impl<'lt> Serialize for MessageSerialize<'lt> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut struct_serializer = serializer.serialize_tuple(self.descriptor.fields().count())?;

        for field_descriptor in self.descriptor.fields() {
            let field = field_descriptor.get_reflect((*self.message).deref());
            struct_serializer.serialize_element(&FieldSerialize::new(field_descriptor, field))?;
        }

        struct_serializer.end()
    }
}

struct FieldSerialize<'lt> {
    descriptor: FieldDescriptor,
    field: ReflectFieldRef<'lt>,
}

impl<'lt> FieldSerialize<'lt> {
    fn new(descriptor: FieldDescriptor, field: ReflectFieldRef<'lt>) -> Self {
        Self { descriptor, field }
    }
}

impl<'lt> Serialize for FieldSerialize<'lt> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match &self.field {
            Optional(optional_field) => match optional_field.value() {
                Some(value) => {
                    serializer.serialize_some(&ValueSerialize::new(&self.descriptor, value))
                }
                _ => serializer.serialize_none(),
            },
            Repeated(repeated_field) => {
                RepeatedValueSerialize::new(&self.descriptor, repeated_field).serialize(serializer)
            }
            Map(_) => todo!("map fields are not supported yet"),
        }
    }
}

struct ValueSerialize<'lt> {
    options: &'lt dyn FieldOptions,
    value: ReflectValueRef<'lt>,
}

impl<'lt> ValueSerialize<'lt> {
    fn new(options: &'lt dyn FieldOptions, value: ReflectValueRef<'lt>) -> Self {
        Self { options, value }
    }

    fn serialize_fixed_size_bytes<S>(
        &self,
        serializer: S,
        size: usize,
        value: &[u8],
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if size != value.len() {
            Err(S::Error::custom(format!(
                "Fixed-size bytes field size mismatch: expected {}, actual {}",
                size,
                value.len()
            )))
        } else {
            let mut tuple = serializer.serialize_tuple(size)?;

            for byte in value {
                tuple.serialize_element(&byte)?;
            }

            tuple.end()
        }
    }

    fn serialize_fixed_size_str<S>(
        &self,
        serializer: S,
        size: usize,
        value: &str,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if size < value.len() {
            Err(S::Error::custom(format!(
                "Fixed-size string field size mismatch: expected {}, actual {}",
                size,
                value.len()
            )))
        } else {
            let mut tuple = serializer.serialize_tuple(size)?;

            for char in value.chars() {
                tuple.serialize_element(&char)?;
            }

            // pad the value with the appropriate number of '\0' chars
            let padding_size = size - value.len();

            for _ in 0..padding_size {
                tuple.serialize_element(&'\0')?;
            }

            tuple.end()
        }
    }

    fn fixed_point_f64<E>(value: f64, encoding: FixedPointEncoding) -> Result<i64, E>
    where
        E: serde::ser::Error,
    {
        let value = ((value - encoding.offset) / encoding.scale).ceil();

        if value > i64::MAX as f64 || value < i64::MIN as f64 {
            return Err(E::custom(
                "i64 overflow when converting f64 to fixed point representation",
            ));
        }

        Ok(value as i64)
    }

    fn fixed_point_f32<E>(value: f32, encoding: FixedPointEncoding) -> Result<i32, E>
    where
        E: serde::ser::Error,
    {
        let value: f64 = (((value as f64) - encoding.offset) / encoding.scale).ceil();

        if value > i32::MAX as f64 || value < i32::MIN as f64 {
            return Err(E::custom(
                "i32 overflow when converting f32 to fixed point representation",
            ));
        }

        Ok(value as i32)
    }
}

impl<'lt> Serialize for ValueSerialize<'lt> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match &self.value {
            U32(value) => match self.options.primitive_byte_size() {
                Some(ByteSize::ONE) => serializer.serialize_u8(
                    (*value).try_into().map_err(|_| S::Error::custom("u8 overflow"))?,
                ),
                Some(ByteSize::TWO) => serializer.serialize_u16(
                    (*value).try_into().map_err(|_| S::Error::custom("u16 overflow"))?,
                ),
                _ => serializer.serialize_u32(*value),
            },
            U64(value) => serializer.serialize_u64(*value),
            I32(value) => match self.options.primitive_byte_size() {
                Some(ByteSize::ONE) => serializer.serialize_i8(
                    (*value).try_into().map_err(|_| S::Error::custom("i8 overflow"))?,
                ),
                Some(ByteSize::TWO) => serializer.serialize_i16(
                    (*value).try_into().map_err(|_| S::Error::custom("i16 overflow"))?,
                ),
                _ => serializer.serialize_i32(*value),
            },
            I64(value) => serializer.serialize_i64(*value),
            F32(value) => {
                if let Some(encoding) = self.options.fixed_point_encoding() {
                    Self::fixed_point_f32(*value, encoding)
                        .and_then(|value| serializer.serialize_i32(value))
                } else {
                    serializer.serialize_f32(*value)
                }
            }
            F64(value) => {
                if let Some(encoding) = self.options.fixed_point_encoding() {
                    Self::fixed_point_f64(*value, encoding)
                        .and_then(|value| serializer.serialize_i64(value))
                } else {
                    serializer.serialize_f64(*value)
                }
            }
            Bytes(value) => {
                if let Some(size) = self.options.variable_type_size().map_err(S::Error::custom)? {
                    self.serialize_fixed_size_bytes(serializer, size, value)
                } else {
                    serializer.serialize_bytes(value)
                }
            }
            Bool(value) => serializer.serialize_bool(*value),
            String(value) => {
                if let Some(size) = self.options.variable_type_size().map_err(S::Error::custom)? {
                    self.serialize_fixed_size_str(serializer, size, value)
                } else {
                    serializer.serialize_str(value)
                }
            }
            Enum(enum_descriptor, value) => match enum_descriptor.enum_byte_size() {
                Some(ByteSize::ONE) => serializer.serialize_i8(
                    (*value).try_into().map_err(|_| S::Error::custom("enum i8 overflow"))?,
                ),
                Some(ByteSize::TWO) => serializer.serialize_i16(
                    (*value).try_into().map_err(|_| S::Error::custom("enum i16 overflow"))?,
                ),
                _ => serializer.serialize_i32(*value),
            },
            Message(value) => {
                MessageSerialize::new(&value.descriptor_dyn(), value).serialize(serializer)
            }
        }
    }
}

struct RepeatedValueSerialize<'lt> {
    options: &'lt dyn FieldOptions,
    value: &'lt ReflectRepeatedRef<'lt>,
}

impl<'lt> RepeatedValueSerialize<'lt> {
    fn new(options: &'lt dyn FieldOptions, value: &'lt ReflectRepeatedRef<'lt>) -> Self {
        Self { options, value }
    }

    fn serialize_as_sequence<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.value.len()))?;

        for element_value in self.value {
            seq.serialize_element(&ValueSerialize::new(self.options, element_value))?;
        }

        seq.end()
    }

    fn serialize_as_tuple<S>(&self, size: usize, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if size != self.value.len() {
            return Err(S::Error::custom(format!(
                "Fixed-size repeated field size mismatch: expected {}, actual {}",
                size,
                self.value.len()
            )));
        }

        let mut tuple = serializer.serialize_tuple(size)?;

        for element_value in self.value {
            tuple.serialize_element(&ValueSerialize::new(self.options, element_value))?;
        }

        tuple.end()
    }
}

impl<'lt> Serialize for RepeatedValueSerialize<'lt> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if let Some(count) = self.options.repeated_field_count().map_err(S::Error::custom)? {
            self.serialize_as_tuple(count, serializer)
        } else {
            self.serialize_as_sequence(serializer)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::protobuf_serialize::MessageSerialize;
    use crate::translator::runtime_message_descriptor::{
        get_file_descriptor, get_message_descriptor,
    };
    use protobuf::reflect::{MessageRef, ReflectValueBox::*};
    use serde_test::{assert_ser_tokens, assert_ser_tokens_error, Token};

    #[test]
    fn serialize_proto_with_supported_primitive_fields() {
        let file_descriptor = get_file_descriptor(
            r#"syntax = 'proto3';

            message TestProto {
                int64 my_i64 = 1;
                int32 my_i32 = 2;
                uint64 my_u64 = 3;
                uint32 my_u32 = 4;
                double my_f64 = 5;
                float my_f32 = 6;
                bool my_bool = 7;
                TestEnum my_enum = 8;
            }

            enum TestEnum {
                A = 0;
                B = 1;
                C = 2;
            }
            "#,
        );

        let message_descriptor = file_descriptor.message_by_full_name(".TestProto").unwrap();
        let enum_descriptor = file_descriptor.enum_by_full_name(".TestEnum").unwrap();
        let mut message = message_descriptor.new_instance();
        for (index, field) in message_descriptor.fields().enumerate() {
            let value = match index {
                0 => I64(-64),
                1 => I32(-32),
                2 => U64(64),
                3 => U32(32),
                4 => F64(6.4),
                5 => F32(3.2),
                6 => Bool(false),
                7 => Enum(enum_descriptor.clone(), 1),
                _ => unreachable!(),
            };

            field.set_singular_field(&mut *message, value);
        }

        let message = &MessageRef::new(&*message);
        let message_serialize = MessageSerialize::new(&message_descriptor, message);

        assert_ser_tokens(
            &message_serialize,
            &[
                Token::Tuple { len: 8 },
                Token::Some,
                Token::I64(-64),
                Token::Some,
                Token::I32(-32),
                Token::Some,
                Token::U64(64),
                Token::Some,
                Token::U32(32),
                Token::Some,
                Token::F64(6.4),
                Token::Some,
                Token::F32(3.2),
                Token::Some,
                Token::Bool(false),
                Token::Some,
                Token::I32(1),
                Token::TupleEnd,
            ],
        )
    }

    #[test]
    fn serialize_proto_with_variable_type_fields() {
        let message_descriptor = get_message_descriptor(
            r#"syntax = 'proto3';

            message TestProto {
                bytes my_bytes = 1;
                string my_string = 2;
            }
            "#,
            ".TestProto",
        );

        let mut message = message_descriptor.new_instance();
        for (index, field) in message_descriptor.fields().enumerate() {
            let value = match index {
                0 => Bytes(vec![1, 2, 3, 4, 5]),
                1 => String("hello".to_string()),
                _ => unreachable!(),
            };

            field.set_singular_field(&mut *message, value);
        }

        let message = &MessageRef::new(&*message);
        let message_serialize = MessageSerialize::new(&message_descriptor, message);

        assert_ser_tokens(
            &message_serialize,
            &[
                Token::Tuple { len: 2 },
                Token::Some,
                Token::Bytes(&[1, 2, 3, 4, 5]),
                Token::Some,
                Token::String("hello"),
                Token::TupleEnd,
            ],
        )
    }

    #[test]
    fn serialize_proto_with_fixed_size_variable_type_fields() {
        let message_descriptor = get_message_descriptor(
            r#"syntax = 'proto3';

            import "syntax/vsidl.proto";

            message TestProto {
                bytes my_bytes = 1 [(sdv.vsidl.variable_type_max_size) = 3];
                string my_string = 2 [(sdv.vsidl.variable_type_max_size) = 10];
            }
            "#,
            ".TestProto",
        );

        let mut message = message_descriptor.new_instance();
        for (index, field) in message_descriptor.fields().enumerate() {
            let value = match index {
                0 => Bytes(vec![1, 2, 3]),
                1 => String("hello".to_string()),
                _ => unreachable!(),
            };

            field.set_singular_field(&mut *message, value);
        }

        let message = &MessageRef::new(&*message);
        let message_serialize = MessageSerialize::new(&message_descriptor, message);

        assert_ser_tokens(
            &message_serialize,
            &[
                Token::Tuple { len: 2 },
                Token::Some,
                Token::Tuple { len: 3 },
                Token::U8(1),
                Token::U8(2),
                Token::U8(3),
                Token::TupleEnd,
                Token::Some,
                Token::Tuple { len: 10 },
                Token::Char('h'),
                Token::Char('e'),
                Token::Char('l'),
                Token::Char('l'),
                Token::Char('o'),
                Token::Char('\0'),
                Token::Char('\0'),
                Token::Char('\0'),
                Token::Char('\0'),
                Token::Char('\0'),
                Token::TupleEnd,
                Token::TupleEnd,
            ],
        )
    }

    #[test]
    fn serialize_proto_with_fixed_size_string_field_overflow() {
        let message_descriptor = get_message_descriptor(
            r#"syntax = 'proto3';

            import "syntax/vsidl.proto";

            message TestProto {
                string my_string = 1 [(sdv.vsidl.variable_type_max_size) = 10];
            }
            "#,
            ".TestProto",
        );

        let mut message = message_descriptor.new_instance();
        for (index, field) in message_descriptor.fields().enumerate() {
            let value = match index {
                0 => String("hello world!".to_string()),
                _ => unreachable!(),
            };

            field.set_singular_field(&mut *message, value);
        }

        let message = &MessageRef::new(&*message);
        let message_serialize = MessageSerialize::new(&message_descriptor, message);

        assert_ser_tokens_error(
            &message_serialize,
            &[
                Token::Tuple { len: 1 },
                Token::Some,
                // serialization should fail at this point
            ],
            "Fixed-size string field size mismatch: expected 10, actual 12",
        )
    }

    #[test]
    fn serialize_proto_with_fixed_size_bytes_field_too_long() {
        let message_descriptor = get_message_descriptor(
            r#"syntax = 'proto3';

            import "syntax/vsidl.proto";

            message TestProto {
                bytes my_bytes = 1 [(sdv.vsidl.variable_type_max_size) = 5];
            }
            "#,
            ".TestProto",
        );

        let mut message = message_descriptor.new_instance();
        for (index, field) in message_descriptor.fields().enumerate() {
            let value = match index {
                0 => Bytes(vec![1, 2, 3, 4, 5, 6]),
                _ => unreachable!(),
            };

            field.set_singular_field(&mut *message, value);
        }

        let message = &MessageRef::new(&*message);
        let message_serialize = MessageSerialize::new(&message_descriptor, message);

        assert_ser_tokens_error(
            &message_serialize,
            &[
                Token::Tuple { len: 1 },
                Token::Some,
                // serialization should fail at this point
            ],
            "Fixed-size bytes field size mismatch: expected 5, actual 6",
        )
    }

    #[test]
    fn serialize_proto_with_fixed_size_bytes_field_too_short() {
        let message_descriptor = get_message_descriptor(
            r#"syntax = 'proto3';

            import "syntax/vsidl.proto";

            message TestProto {
                bytes my_bytes = 1 [(sdv.vsidl.variable_type_max_size) = 9];
            }
            "#,
            ".TestProto",
        );

        let mut message = message_descriptor.new_instance();
        for (index, field) in message_descriptor.fields().enumerate() {
            let value = match index {
                0 => Bytes(vec![1, 2, 3, 4, 5, 6]),
                _ => unreachable!(),
            };

            field.set_singular_field(&mut *message, value);
        }

        let message = &MessageRef::new(&*message);
        let message_serialize = MessageSerialize::new(&message_descriptor, message);

        assert_ser_tokens_error(
            &message_serialize,
            &[
                Token::Tuple { len: 1 },
                Token::Some,
                // serialization should fail at this point
            ],
            "Fixed-size bytes field size mismatch: expected 9, actual 6",
        )
    }

    #[test]
    fn serialize_proto_with_nested_messages() {
        let file_descriptor = get_file_descriptor(
            r#"syntax = 'proto3';

            message OuterProto {
                uint64 my_u64 = 1;
                InnerProto my_msg = 2;
            }

            message InnerProto {
                InnermostProto my_msg = 1;
                int64 my_i64 = 2;
            }

            message InnermostProto {
                float my_f32 = 1;
            }
            "#,
        );

        let outer_descriptor = file_descriptor.message_by_full_name(".OuterProto").unwrap();
        let inner_descriptor = file_descriptor.message_by_full_name(".InnerProto").unwrap();
        let innermost_descriptor = file_descriptor.message_by_full_name(".InnermostProto").unwrap();

        let mut outer_message = outer_descriptor.new_instance();
        for (index, field) in outer_descriptor.fields().enumerate() {
            let value = match index {
                0 => U64(64),
                1 => {
                    let mut inner_message = inner_descriptor.new_instance();
                    for (index, field) in inner_descriptor.fields().enumerate() {
                        let value = match index {
                            1 => I64(10064),
                            0 => {
                                let mut innermost_message = innermost_descriptor.new_instance();
                                for (index, field) in innermost_descriptor.fields().enumerate() {
                                    let value = match index {
                                        0 => F32(100.32),
                                        _ => unreachable!(),
                                    };

                                    field.set_singular_field(&mut *innermost_message, value);
                                }
                                Message(innermost_message)
                            }
                            _ => unreachable!(),
                        };

                        field.set_singular_field(&mut *inner_message, value);
                    }
                    Message(inner_message)
                }
                _ => unreachable!(),
            };

            field.set_singular_field(&mut *outer_message, value);
        }

        let outer_message = &MessageRef::new(&*outer_message);
        let message_serialize = MessageSerialize::new(&outer_descriptor, outer_message);

        assert_ser_tokens(
            &message_serialize,
            &[
                Token::Tuple { len: 2 },
                Token::Some,
                Token::U64(64),
                Token::Some,
                Token::Tuple { len: 2 },
                Token::Some,
                Token::Tuple { len: 1 },
                Token::Some,
                Token::F32(100.32),
                Token::TupleEnd,
                Token::Some,
                Token::I64(10064),
                Token::TupleEnd,
                Token::TupleEnd,
            ],
        )
    }

    #[test]
    fn serialize_proto_with_repeated_fields() {
        let file_descriptor = get_file_descriptor(
            r#"syntax = 'proto3';

            message TestProto {
                repeated int64 my_i64 = 1;
                repeated TestEnum my_enum = 2;
                repeated double my_f64 = 3;
            }

            enum TestEnum {
                A = 0;
                B = 1;
                C = 2;
            }
            "#,
        );

        let message_descriptor = file_descriptor.message_by_full_name(".TestProto").unwrap();
        let enum_descriptor = file_descriptor.enum_by_full_name(".TestEnum").unwrap();
        let mut message = message_descriptor.new_instance();
        for (index, field) in message_descriptor.fields().enumerate() {
            let values = match index {
                0 => {
                    vec![]
                }
                1 => {
                    vec![Enum(enum_descriptor.clone(), 1)]
                }
                2 => {
                    vec![F64(-6.4), F64(-64.0), F64(-0.001)]
                }
                _ => unreachable!(),
            };

            for value in values {
                field.mut_repeated(&mut *message).push(value);
            }
        }

        let message = &MessageRef::new(&*message);
        let message_serialize = MessageSerialize::new(&message_descriptor, message);

        assert_ser_tokens(
            &message_serialize,
            &[
                Token::Tuple { len: 3 },
                Token::Seq { len: Some(0) },
                Token::SeqEnd,
                Token::Seq { len: Some(1) },
                Token::I32(1),
                Token::SeqEnd,
                Token::Seq { len: Some(3) },
                Token::F64(-6.4),
                Token::F64(-64.0),
                Token::F64(-0.001),
                Token::SeqEnd,
                Token::TupleEnd,
            ],
        )
    }

    #[test]
    fn serialize_proto_with_fixed_size_repeated_fields() {
        let file_descriptor = get_file_descriptor(
            r#"syntax = 'proto3';

            import "syntax/vsidl.proto";

            message TestProto {
                repeated double my_f64 = 1 [(sdv.vsidl.repeated_field_max_count) = 3];
            }
            "#,
        );

        let message_descriptor = file_descriptor.message_by_full_name(".TestProto").unwrap();
        let mut message = message_descriptor.new_instance();
        for (index, field) in message_descriptor.fields().enumerate() {
            let values = match index {
                0 => {
                    vec![F64(-6.4), F64(-64.0), F64(-0.001)]
                }
                _ => unreachable!(),
            };

            for value in values {
                field.mut_repeated(&mut *message).push(value);
            }
        }

        let message = &MessageRef::new(&*message);
        let message_serialize = MessageSerialize::new(&message_descriptor, message);

        assert_ser_tokens(
            &message_serialize,
            &[
                Token::Tuple { len: 1 },
                Token::Tuple { len: 3 },
                Token::F64(-6.4),
                Token::F64(-64.0),
                Token::F64(-0.001),
                Token::TupleEnd,
                Token::TupleEnd,
            ],
        )
    }

    #[test]
    fn serialize_proto_with_fixed_size_repeated_field_too_long() {
        let message_descriptor = get_message_descriptor(
            r#"syntax = 'proto3';

            import "syntax/vsidl.proto";

            message TestProto {
                repeated int64 my_i64 = 1 [(sdv.vsidl.repeated_field_max_count) = 1];
            }
            "#,
            ".TestProto",
        );

        let mut message = message_descriptor.new_instance();
        for (index, field) in message_descriptor.fields().enumerate() {
            let values = match index {
                0 => {
                    vec![I64(1), I64(2)]
                }
                _ => unreachable!(),
            };

            for value in values {
                field.mut_repeated(&mut *message).push(value);
            }
        }

        let message = &MessageRef::new(&*message);
        let message_serialize = MessageSerialize::new(&message_descriptor, message);

        assert_ser_tokens_error(
            &message_serialize,
            &[
                Token::Tuple { len: 1 },
                // serialization should fail at this point
            ],
            "Fixed-size repeated field size mismatch: expected 1, actual 2",
        )
    }

    #[test]
    fn serialize_proto_with_fixed_size_repeated_field_too_short() {
        let message_descriptor = get_message_descriptor(
            r#"syntax = 'proto3';

            import "syntax/vsidl.proto";

            message TestProto {
                repeated int64 my_i64 = 1 [(sdv.vsidl.repeated_field_max_count) = 3];
            }
            "#,
            ".TestProto",
        );

        let mut message = message_descriptor.new_instance();
        for (index, field) in message_descriptor.fields().enumerate() {
            let values = match index {
                0 => {
                    vec![I64(1), I64(2)]
                }
                _ => unreachable!(),
            };

            for value in values {
                field.mut_repeated(&mut *message).push(value);
            }
        }

        let message = &MessageRef::new(&*message);
        let message_serialize = MessageSerialize::new(&message_descriptor, message);

        assert_ser_tokens_error(
            &message_serialize,
            &[
                Token::Tuple { len: 1 },
                // serialization should fail at this point
            ],
            "Fixed-size repeated field size mismatch: expected 3, actual 2",
        )
    }

    #[test]
    fn serialize_proto_with_variable_type_repeated_fields() {
        let message_descriptor = get_message_descriptor(
            r#"syntax = 'proto3';

            import "syntax/vsidl.proto";

            message TestProto {
                repeated string my_string = 1
                    [(sdv.vsidl.variable_type_max_size) = 10, (sdv.vsidl.repeated_field_max_count) = 2];
            }
            "#,
            ".TestProto",
        );

        let mut message = message_descriptor.new_instance();
        for (index, field) in message_descriptor.fields().enumerate() {
            let values = match index {
                0 => vec![String("hello".to_string()), String("world!".to_string())],
                _ => unreachable!(),
            };

            for value in values {
                field.mut_repeated(&mut *message).push(value);
            }
        }

        let message = &MessageRef::new(&*message);
        let message_serialize = MessageSerialize::new(&message_descriptor, message);

        assert_ser_tokens(
            &message_serialize,
            &[
                Token::Tuple { len: 1 },
                Token::Tuple { len: 2 },
                Token::Tuple { len: 10 },
                Token::Char('h'),
                Token::Char('e'),
                Token::Char('l'),
                Token::Char('l'),
                Token::Char('o'),
                Token::Char('\0'),
                Token::Char('\0'),
                Token::Char('\0'),
                Token::Char('\0'),
                Token::Char('\0'),
                Token::TupleEnd,
                Token::Tuple { len: 10 },
                Token::Char('w'),
                Token::Char('o'),
                Token::Char('r'),
                Token::Char('l'),
                Token::Char('d'),
                Token::Char('!'),
                Token::Char('\0'),
                Token::Char('\0'),
                Token::Char('\0'),
                Token::Char('\0'),
                Token::TupleEnd,
                Token::TupleEnd,
                Token::TupleEnd,
            ],
        )
    }

    #[test]
    fn serialize_int_fields_with_byte_size_options() {
        let message_descriptor = get_message_descriptor(
            r#"syntax = 'proto3';

            import "syntax/diag_serde.proto";

            message TestProto {
                uint32 one_byte_u32 = 1 [(sdv.diag.primitive_byte_size) = ONE];
                uint32 two_bytes_u32 = 2 [(sdv.diag.primitive_byte_size) = TWO];
                int32 one_byte_i32 = 3 [(sdv.diag.primitive_byte_size) = ONE];
                int32 two_bytes_i32 = 4 [(sdv.diag.primitive_byte_size) = TWO];
            }
            "#,
            ".TestProto",
        );

        let mut message = message_descriptor.new_instance();
        for (index, field) in message_descriptor.fields().enumerate() {
            let value = match index {
                0 => U32(128),
                1 => U32(1024),
                2 => I32(-120),
                3 => I32(-510),
                _ => unreachable!(),
            };

            field.set_singular_field(&mut *message, value);
        }

        let message = &MessageRef::new(&*message);
        let message_serialize = MessageSerialize::new(&message_descriptor, message);

        assert_ser_tokens(
            &message_serialize,
            &[
                Token::Tuple { len: 4 },
                Token::Some,
                Token::U8(128),
                Token::Some,
                Token::U16(1024),
                Token::Some,
                Token::I8(-120),
                Token::Some,
                Token::I16(-510),
                Token::TupleEnd,
            ],
        )
    }

    #[test]
    fn serialize_float_fields_with_fixed_point_options() {
        let message_descriptor = get_message_descriptor(
            r#"syntax = 'proto3';

            import "syntax/diag_serde.proto";
            import "syntax/vsidl.proto";

            message TestProto {
                repeated float my_fixed_float = 1 [(sdv.diag.fixed_point_encoding) = {
                    scale: 0.001
                    offset: 20
                }, (sdv.vsidl.repeated_field_max_count) = 2];
                repeated double my_fixed_double = 2 [(sdv.diag.fixed_point_encoding) = {
                    scale: 0.0001
                    offset: 0
                }, (sdv.vsidl.repeated_field_max_count) = 2];
            }
            "#,
            ".TestProto",
        );

        let mut message = message_descriptor.new_instance();
        for (index, field) in message_descriptor.fields().enumerate() {
            let values = match index {
                0 => [F32(20.0132), F32(19.8947)],
                1 => [F64(0.12345), F64(-0.12345)],
                _ => unreachable!(),
            };

            for value in values {
                field.mut_repeated(&mut *message).push(value);
            }
        }

        let message = &MessageRef::new(&*message);
        let message_serialize = MessageSerialize::new(&message_descriptor, message);

        assert_ser_tokens(
            &message_serialize,
            &[
                Token::Tuple { len: 2 },
                Token::Tuple { len: 2 },
                Token::I32(14),
                Token::I32(-105),
                Token::TupleEnd,
                Token::Tuple { len: 2 },
                Token::I64(1235),
                Token::I64(-1234),
                Token::TupleEnd,
                Token::TupleEnd,
            ],
        )
    }

    #[test]
    fn serialize_float_field_with_fixed_point_overflow() {
        let message_descriptor = get_message_descriptor(
            r#"syntax = 'proto3';

            import "syntax/diag_serde.proto";
            import "syntax/vsidl.proto";

            message TestProto {
                float my_fixed_float = 1 [(sdv.diag.fixed_point_encoding) = {
                    scale: 1
                    offset: 0
                }];
            }
            "#,
            ".TestProto",
        );

        let mut message = message_descriptor.new_instance();
        for (index, field) in message_descriptor.fields().enumerate() {
            let value = match index {
                0 => F32(i32::MAX as f32 + 1.0),
                _ => unreachable!(),
            };

            field.set_singular_field(&mut *message, value);
        }

        let message = &MessageRef::new(&*message);
        let message_serialize = MessageSerialize::new(&message_descriptor, message);

        assert_ser_tokens_error(
            &message_serialize,
            &[Token::Tuple { len: 1 }, Token::Some],
            "i32 overflow when converting f32 to fixed point representation",
        )
    }

    #[test]
    fn serialize_double_field_with_fixed_point_overflow() {
        let message_descriptor = get_message_descriptor(
            r#"syntax = 'proto3';

            import "syntax/diag_serde.proto";
            import "syntax/vsidl.proto";

            message TestProto {
                double my_fixed_double = 1 [(sdv.diag.fixed_point_encoding) = {
                    scale: 1
                    offset: 0
                }];
            }
            "#,
            ".TestProto",
        );

        let mut message = message_descriptor.new_instance();
        for (index, field) in message_descriptor.fields().enumerate() {
            let value = match index {
                0 => F64(i64::MIN as f64 - 10000.0),
                _ => unreachable!(),
            };

            field.set_singular_field(&mut *message, value);
        }

        let message = &MessageRef::new(&*message);
        let message_serialize = MessageSerialize::new(&message_descriptor, message);

        assert_ser_tokens_error(
            &message_serialize,
            &[Token::Tuple { len: 1 }, Token::Some],
            "i64 overflow when converting f64 to fixed point representation",
        )
    }

    #[test]
    fn serialize_proto_with_enum_with_byte_size() {
        let file_descriptor = get_file_descriptor(
            r#"syntax = 'proto3';

            import "syntax/diag_serde.proto";

            message TestProto {
                TestEnumOne my_1_byte_enum = 1;
                TestEnumTwo my_2_byte_enum = 2;
            }

            enum TestEnumOne {
                option (sdv.diag.enum_byte_size) = ONE;

                A = 0;
            }

            enum TestEnumTwo {
                option (sdv.diag.enum_byte_size) = TWO;

                A = 0;
            }
            "#,
        );

        let message_descriptor = file_descriptor.message_by_full_name(".TestProto").unwrap();
        let enum_descriptor1 = file_descriptor.enum_by_full_name(".TestEnumOne").unwrap();
        let enum_descriptor2 = file_descriptor.enum_by_full_name(".TestEnumTwo").unwrap();
        let mut message = message_descriptor.new_instance();
        for (index, field) in message_descriptor.fields().enumerate() {
            let value = match index {
                0 => Enum(enum_descriptor1.clone(), 0),
                1 => Enum(enum_descriptor2.clone(), 0),
                _ => unreachable!(),
            };

            field.set_singular_field(&mut *message, value);
        }

        let message = &MessageRef::new(&*message);
        let message_serialize = MessageSerialize::new(&message_descriptor, message);

        assert_ser_tokens(
            &message_serialize,
            &[
                Token::Tuple { len: 2 },
                Token::Some,
                Token::I8(0),
                Token::Some,
                Token::I16(0),
                Token::TupleEnd,
            ],
        )
    }
}
