// Copyright 2024 Google LLC
use protobuf::reflect::{
    FieldDescriptor, MessageDescriptor, MessageRef,
    ReflectFieldRef::{self, *},
    ReflectValueRef::{self, *},
};
use serde::ser::SerializeTuple;
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
            Repeated(_) => todo!("repeated fields are not supported yet"),
            Map(_) => todo!("map fields are not supported yet"),
        }
    }
}

struct ValueSerialize<'lt> {
    _descriptor: &'lt FieldDescriptor,
    value: ReflectValueRef<'lt>,
}

impl<'lt> ValueSerialize<'lt> {
    fn new(_descriptor: &'lt FieldDescriptor, value: ReflectValueRef<'lt>) -> Self {
        Self { _descriptor, value }
    }
}

impl<'lt> Serialize for ValueSerialize<'lt> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match &self.value {
            U32(value) => serializer.serialize_u32(*value),
            U64(value) => serializer.serialize_u64(*value),
            I32(value) => serializer.serialize_i32(*value),
            I64(value) => serializer.serialize_i64(*value),
            F32(value) => serializer.serialize_f32(*value),
            F64(value) => serializer.serialize_f64(*value),
            Bytes(value) => serializer.serialize_bytes(value),
            Bool(value) => serializer.serialize_bool(*value),
            String(value) => serializer.serialize_str(value),
            Enum(_, value) => serializer.serialize_i32(*value),
            Message(value) => {
                MessageSerialize::new(&value.descriptor_dyn(), value).serialize(serializer)
            }
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
    use serde_test::{assert_ser_tokens, Token};

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
    fn serialize_proto_with_variable_size_fields() {
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
}
