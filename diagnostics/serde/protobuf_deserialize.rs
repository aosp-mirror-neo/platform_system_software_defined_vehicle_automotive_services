// Copyright 2024 Google LLC
use protobuf::reflect::{
    EnumDescriptor, FieldDescriptor, MessageDescriptor,
    ReflectValueBox::{self, *},
    RuntimeFieldType::*,
    RuntimeType,
};
use protobuf::MessageDyn;
use serde::de::{DeserializeSeed, Error, SeqAccess, Visitor};
use serde::Deserializer;
use std::fmt;

// `DeserializeSeed` implementation for deserialization from proto message bytes into
// `Box<dyn MessageDyn>` - a dynamic in-memory representation of a proto message.
//
// We use `DeserializeSeed` since deserialization is driven by `MessageDescriptor`, and
// `DeserializeSeed` allows it to be passed to `deserialize` via `self`.
//
// Delegates deserialization of individual fields to `FieldDeserializeSeed`.
pub(crate) struct MessageDeserializeSeed<'lt> {
    message_descriptor: &'lt MessageDescriptor,
}

impl<'lt> MessageDeserializeSeed<'lt> {
    pub fn new(message_descriptor: &'lt MessageDescriptor) -> Self {
        MessageDeserializeSeed { message_descriptor }
    }
}

impl<'de, 'lt> DeserializeSeed<'de> for MessageDeserializeSeed<'lt> {
    type Value = Box<dyn MessageDyn>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct MessageVisitor<'lt> {
            descriptor: &'lt MessageDescriptor,
        }

        impl<'lt> MessageVisitor<'lt> {
            fn new(descriptor: &'lt MessageDescriptor) -> Self {
                Self { descriptor }
            }
        }

        impl<'de, 'lt> Visitor<'de> for MessageVisitor<'lt> {
            type Value = Box<dyn MessageDyn>;

            fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                write!(fmt, "Protobuf message as a tuple")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut message = self.descriptor.new_instance();

                for field in self.descriptor.fields() {
                    // `FieldDeserializeSeed::deserialize`` returns () and deserializes
                    // the field directly into the `message` passed to its constructor.
                    let Some(()) =
                        seq.next_element_seed(FieldDeserializeSeed::new(&field, &mut message))?
                    else {
                        return Err(A::Error::custom("Expected more protobuf message fields"));
                    };
                }

                Ok(message)
            }
        }

        deserializer.deserialize_tuple(
            self.message_descriptor.proto().field.len(),
            MessageVisitor::new(self.message_descriptor),
        )
    }
}

// `DeserializeSeed` for an individual message field represented by
// `self.field_descriptor`.
//
// Field value is deserialized directly into `self.message`.
//
// Responsible for figuring out how many values of what type has to be deserialized
// depending on type of field (optional / repeated / map) and delegating to
// ValueDeserializeSeed.
struct FieldDeserializeSeed<'lt> {
    field_descriptor: &'lt FieldDescriptor,
    message: &'lt mut Box<dyn MessageDyn>,
}

impl<'lt> FieldDeserializeSeed<'lt> {
    fn new(field_descriptor: &'lt FieldDescriptor, message: &'lt mut Box<dyn MessageDyn>) -> Self {
        Self { field_descriptor, message }
    }
}

impl<'de, 'lt> DeserializeSeed<'de> for FieldDeserializeSeed<'lt> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        /// Singular protobuf field visitor.
        ///
        /// Forwards to `ValueDeserializeSeed` if the field is present (Some(_)),
        /// or skips the field if it's empty (None).
        struct OptionVisitor<'lt> {
            descriptor: &'lt FieldDescriptor,
            runtime_type: RuntimeType,
        }

        impl<'lt> OptionVisitor<'lt> {
            fn new(descriptor: &'lt FieldDescriptor, runtime_type: RuntimeType) -> Self {
                Self { descriptor, runtime_type }
            }
        }

        impl<'de, 'lt> Visitor<'de> for OptionVisitor<'lt> {
            type Value = Option<ReflectValueBox>;

            fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                write!(fmt, "Optional value")
            }

            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                Ok(Some(
                    ValueDeserializeSeed::new(self.descriptor, &self.runtime_type)
                        .deserialize(deserializer)?,
                ))
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(None)
            }
        }

        struct RepeatedFieldVisitor<'lt> {
            message: &'lt mut Box<dyn MessageDyn>,
            descriptor: &'lt FieldDescriptor,
            runtime_type: RuntimeType,
        }

        impl<'lt> RepeatedFieldVisitor<'lt> {
            fn new(
                message: &'lt mut Box<dyn MessageDyn>,
                descriptor: &'lt FieldDescriptor,
                runtime_type: RuntimeType,
            ) -> Self {
                Self { message, descriptor, runtime_type }
            }
        }

        impl<'de, 'lt> Visitor<'de> for RepeatedFieldVisitor<'lt> {
            type Value = ();

            fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                write!(fmt, "Repeated value")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut repeated_field = self.descriptor.mut_repeated(&mut **self.message);

                while let Some(element_value) = seq.next_element_seed(ValueDeserializeSeed::new(
                    self.descriptor,
                    &self.runtime_type,
                ))? {
                    repeated_field.push(element_value)
                }

                Ok(())
            }
        }

        match self.field_descriptor.runtime_field_type() {
            Singular(runtime_type) => {
                // Singular protobuf fields are optional, thus we deserialize using
                // `OptionVisitor`.
                let field_value = deserializer
                    .deserialize_option(OptionVisitor::new(self.field_descriptor, runtime_type))?;

                if let Some(field_value) = field_value {
                    self.field_descriptor.set_singular_field(&mut **self.message, field_value);
                }
            }
            Repeated(runtime_type) => {
                deserializer.deserialize_seq(RepeatedFieldVisitor::new(
                    self.message,
                    self.field_descriptor,
                    runtime_type,
                ))?;
            }
            Map(_, _) => todo!("map fields are not supported yet"),
        };
        Ok(())
    }
}

// Deserializer for individual value within the protobuf field.
//
// This may be:
// - value of the optional field
// - single element within a repeated field
// - single key or value within a map field
struct ValueDeserializeSeed<'lt> {
    _descriptor: &'lt FieldDescriptor,
    runtime_type: &'lt RuntimeType,
}

impl<'lt> ValueDeserializeSeed<'lt> {
    fn new(_descriptor: &'lt FieldDescriptor, runtime_type: &'lt RuntimeType) -> Self {
        Self { _descriptor, runtime_type }
    }
}

impl<'de, 'lt> DeserializeSeed<'de> for ValueDeserializeSeed<'lt> {
    type Value = ReflectValueBox;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        match self.runtime_type {
            RuntimeType::I32 => {
                struct I32ValueVisitor;

                impl<'de> Visitor<'de> for I32ValueVisitor {
                    type Value = ReflectValueBox;

                    fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                        write!(fmt, "single I32 value")
                    }

                    fn visit_i32<E>(self, value: i32) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        Ok(I32(value))
                    }

                    // Deserializer implementations might ignore `deserialize_i32` hint.
                    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        let value = value.try_into().map_err(|_| E::custom("i32 overflow"))?;

                        self.visit_i32(value)
                    }

                    // Deserializer implementations might ignore `deserialize_i32` hint.
                    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        let value = value.try_into().map_err(|_| E::custom("i32 overflow"))?;

                        self.visit_i32(value)
                    }
                }

                deserializer.deserialize_i32(I32ValueVisitor)
            }
            RuntimeType::I64 => {
                struct I64ValueVisitor;

                impl<'de> Visitor<'de> for I64ValueVisitor {
                    type Value = ReflectValueBox;

                    fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                        write!(fmt, "single I64 value")
                    }

                    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        Ok(I64(value))
                    }

                    // Deserializer implementations might ignore `deserialize_i32` hint.
                    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        let value = value.try_into().map_err(|_| E::custom("i64 overflow"))?;

                        self.visit_i64(value)
                    }
                }

                deserializer.deserialize_i64(I64ValueVisitor)
            }
            RuntimeType::U32 => {
                struct U32ValueVisitor;

                impl<'de> Visitor<'de> for U32ValueVisitor {
                    type Value = ReflectValueBox;

                    fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                        write!(fmt, "single U32 value")
                    }

                    fn visit_u32<E>(self, value: u32) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        Ok(U32(value))
                    }

                    // Deserializer implementations might ignore `deserialize_u32` hint.
                    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        let value = value.try_into().map_err(|_| E::custom("u32 overflow"))?;

                        self.visit_u32(value)
                    }

                    // Deserializer implementations might ignore `deserialize_u32` hint.
                    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        let value = value.try_into().map_err(|_| E::custom("u32 overflow"))?;

                        self.visit_u32(value)
                    }
                }

                deserializer.deserialize_u32(U32ValueVisitor)
            }
            RuntimeType::U64 => {
                struct U64ValueVisitor;

                impl<'de> Visitor<'de> for U64ValueVisitor {
                    type Value = ReflectValueBox;

                    fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                        write!(fmt, "single U64 value")
                    }

                    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        Ok(U64(value))
                    }

                    // Deserializer implementations might ignore `deserialize_u32` hint.
                    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        let value = value.try_into().map_err(|_| E::custom("u64 overflow"))?;

                        self.visit_u64(value)
                    }
                }

                deserializer.deserialize_u64(U64ValueVisitor)
            }
            RuntimeType::F32 => {
                struct F32ValueVisitor;

                impl<'de> Visitor<'de> for F32ValueVisitor {
                    type Value = ReflectValueBox;

                    fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                        write!(fmt, "single F32 value")
                    }

                    fn visit_f32<E>(self, value: f32) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        Ok(F32(value))
                    }

                    // Deserializer implementations might ignore `deserialize_f32` hint.
                    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        Ok(F32(value as f32))
                    }
                }

                deserializer.deserialize_f32(F32ValueVisitor)
            }
            RuntimeType::F64 => {
                struct F64ValueVisitor;

                impl<'de> Visitor<'de> for F64ValueVisitor {
                    type Value = ReflectValueBox;

                    fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                        write!(fmt, "single F64 value")
                    }

                    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        Ok(F64(value))
                    }
                }

                deserializer.deserialize_f64(F64ValueVisitor)
            }
            RuntimeType::Bool => {
                struct BoolValueVisitor;

                impl<'de> Visitor<'de> for BoolValueVisitor {
                    type Value = ReflectValueBox;

                    fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                        write!(fmt, "single Bool value")
                    }

                    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        Ok(Bool(value))
                    }
                }

                deserializer.deserialize_bool(BoolValueVisitor)
            }
            RuntimeType::String => {
                struct StringValueVisitor;

                impl<'de> Visitor<'de> for StringValueVisitor {
                    type Value = ReflectValueBox;

                    fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                        write!(fmt, "single variable-size String value")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        Ok(String(value.to_string()))
                    }
                }

                deserializer.deserialize_string(StringValueVisitor)
            }
            RuntimeType::VecU8 => {
                struct VecU8ValueVisitor;

                impl<'de> Visitor<'de> for VecU8ValueVisitor {
                    type Value = ReflectValueBox;

                    fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                        write!(fmt, "single variable-size Bytes value")
                    }

                    fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        let mut owned_value = Vec::new();
                        owned_value.extend_from_slice(value);
                        Ok(Bytes(owned_value))
                    }
                }

                deserializer.deserialize_byte_buf(VecU8ValueVisitor)
            }
            RuntimeType::Enum(enum_descriptor) => {
                struct EnumValueVisitor<'lt>(&'lt EnumDescriptor);

                impl<'de, 'lt> Visitor<'de> for EnumValueVisitor<'lt> {
                    type Value = ReflectValueBox;

                    fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                        write!(fmt, "single I32 value representing Enum")
                    }

                    fn visit_i32<E>(self, variant_index: i32) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        if (variant_index as usize) < self.0.proto().value.len() {
                            Ok(Enum(self.0.clone(), variant_index))
                        } else {
                            Err(E::custom("enum variant index is out of bounds"))
                        }
                    }

                    // Deserializer implementations might ignore `deserialize_i32` hint.
                    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        let variant_index =
                            value.try_into().map_err(|_| E::custom("i32 overflow"))?;

                        self.visit_i32(variant_index)
                    }
                }

                deserializer.deserialize_i32(EnumValueVisitor(enum_descriptor))
            }
            RuntimeType::Message(message_descriptor) => {
                let message =
                    MessageDeserializeSeed::new(message_descriptor).deserialize(deserializer)?;
                Ok(Message(message))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::protobuf_deserialize::MessageDeserializeSeed;
    use crate::translator::runtime_message_descriptor::get_file_descriptor;
    use lazy_static::lazy_static;
    use protobuf::reflect::{FileDescriptor, ReflectValueBox::*};
    use protobuf::MessageDyn;
    use serde::de::DeserializeSeed;
    use serde::{Deserialize, Deserializer};
    use serde_test::{assert_de_tokens, assert_de_tokens_error, Token};

    // Simple test that avoids using `serde_test` utilities and related workarounds to
    // showcase the usage of `MessageDeserializeSeed`.
    #[test]
    fn deserialize_proto_with_supported_primitive_fields_from_json() {
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
            }
            "#,
        );

        let message_descriptor = file_descriptor.message_by_full_name(".TestProto").unwrap();
        let mut expected_message = message_descriptor.new_instance();
        for (index, field) in message_descriptor.fields().enumerate() {
            let value = match index {
                0 => I64(-64),
                1 => I32(-32),
                2 => U64(64),
                3 => U32(32),
                4 => F64(6.4),
                5 => F32(3.2),
                6 => Bool(false),
                _ => unreachable!(),
            };

            field.set_singular_field(&mut *expected_message, value);
        }

        let deserialize_seed = MessageDeserializeSeed::new(&message_descriptor);
        let mut json_deserializer = serde_json::de::Deserializer::from_str(
            r#"
            [
                -64,
                -32,
                64,
                32,
                6.4,
                3.2,
                false
            ]
            "#,
        );
        let actual_message = deserialize_seed.deserialize(&mut json_deserializer).unwrap();

        assert_eq!(&expected_message, &actual_message)
    }

    /// Workaround to enable usage of `serde_test` utilities with deserialization implemented
    /// via `DeserializeSeed` (rather than `Deserialize`).
    #[macro_export]
    macro_rules! prepare_descriptors_and_deserializer {
        ($proto_file:expr, $message_name:expr, $file_descriptor:ident, $message_descriptor:ident) => {
            // Single instantiation of `FileDescriptor` to ensure both expected and
            // actual deserialized proto messages use the same descriptor instance.
            // This is required for correct equality check.
            lazy_static! {
                static ref FILE_DESCRIPTOR: FileDescriptor = get_file_descriptor($proto_file);
            }

            // Definition of file descriptor variable to be used in test.
            let $file_descriptor = &FILE_DESCRIPTOR;

            let $message_descriptor = FILE_DESCRIPTOR.message_by_full_name($message_name).unwrap();

            #[derive(Debug)]
            struct DeserializeSeedWrapper {
                message: Box<dyn MessageDyn>,
            }

            impl PartialEq for DeserializeSeedWrapper {
                fn eq(&self, other: &Self) -> bool {
                    &self.message == &other.message
                }
            }

            impl<'de> Deserialize<'de> for DeserializeSeedWrapper {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: Deserializer<'de>,
                {
                    let descriptor = FILE_DESCRIPTOR.message_by_full_name($message_name).unwrap();

                    Ok(Self {
                        message: MessageDeserializeSeed::new(&descriptor)
                            .deserialize(deserializer)?,
                    })
                }
            }
        };
    }

    #[test]
    fn deserialize_proto_with_supported_primitive_fields() {
        prepare_descriptors_and_deserializer!(
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
            ".TestProto",
            file_descriptor,
            message_descriptor
        );

        let mut expected_message = message_descriptor.new_instance();
        let enum_descriptor = file_descriptor.enum_by_full_name(".TestEnum").unwrap();
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

            field.set_singular_field(&mut *expected_message, value);
        }

        let message_deserialize = DeserializeSeedWrapper { message: expected_message };

        assert_de_tokens(
            &message_deserialize,
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
    fn deserialize_proto_with_variable_size_fields() {
        prepare_descriptors_and_deserializer!(
            r#"syntax = 'proto3';

            message TestProto {
                bytes my_bytes = 1;
                bytes my_owned_bytes = 2;
                bytes my_borrowed_bytes = 3;
                string my_string = 4;
                string my_owned_string = 5;
                string my_borrowed_string = 6;
            }
            "#,
            ".TestProto",
            _file_descriptor,
            message_descriptor
        );

        let mut expected_message = message_descriptor.new_instance();
        for (index, field) in message_descriptor.fields().enumerate() {
            let value = match index {
                0 => Bytes(vec![1, 2]),
                1 => Bytes(vec![3, 4]),
                2 => Bytes(vec![5, 6]),
                3 => String("hello".to_string()),
                4 => String("world".to_string()),
                5 => String("!".to_string()),
                _ => unreachable!(),
            };

            field.set_singular_field(&mut *expected_message, value);
        }

        let message_deserialize = DeserializeSeedWrapper { message: expected_message };

        assert_de_tokens(
            &message_deserialize,
            &[
                Token::Tuple { len: 6 },
                Token::Some,
                Token::Bytes(&[1, 2]),
                Token::Some,
                Token::ByteBuf(&[3, 4]),
                Token::Some,
                Token::BorrowedBytes(&[5, 6]),
                Token::Some,
                Token::Str("hello"),
                Token::Some,
                Token::String("world"),
                Token::Some,
                Token::BorrowedStr("!"),
                Token::TupleEnd,
            ],
        )
    }

    #[test]
    fn deserialize_proto_with_nested_messages() {
        prepare_descriptors_and_deserializer!(
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
            ".OuterProto",
            file_descriptor,
            outer_descriptor
        );

        let inner_descriptor = file_descriptor.message_by_full_name(".InnerProto").unwrap();
        let innermost_descriptor = file_descriptor.message_by_full_name(".InnermostProto").unwrap();

        let mut expected_message = outer_descriptor.new_instance();
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

            field.set_singular_field(&mut *expected_message, value);
        }

        let message_deserialize = DeserializeSeedWrapper { message: expected_message };

        assert_de_tokens(
            &message_deserialize,
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
    fn deserialize_proto_empty_optionals() {
        prepare_descriptors_and_deserializer!(
            r#"syntax = 'proto3';

            message TestProto {
                int64 my_i64 = 1;
                int32 my_i32 = 2;
                uint64 my_u64 = 3;
                uint32 my_u32 = 4;
                double my_f64 = 5;
                float my_f32 = 6;
                bool my_bool = 7;
                InnerProto my_msg = 8;
                string my_string = 9;
                bytes my_bytes = 10;
            }

            message InnerProto {
                int64 my_i64 = 1;
            }
            "#,
            ".TestProto",
            _file_descriptor,
            message_descriptor
        );

        let expected_message = message_descriptor.new_instance();
        let message_deserialize = DeserializeSeedWrapper { message: expected_message };

        assert_de_tokens(
            &message_deserialize,
            &[
                Token::Tuple { len: 10 },
                Token::None,
                Token::None,
                Token::None,
                Token::None,
                Token::None,
                Token::None,
                Token::None,
                Token::None,
                Token::None,
                Token::None,
                Token::TupleEnd,
            ],
        );
    }

    #[test]
    fn deserialize_proto_missing_fields() {
        prepare_descriptors_and_deserializer!(
            r#"syntax = 'proto3';

            message TestProto {
                int64 my_i64 = 1;
                int32 my_i32 = 2;
                uint64 my_u64 = 3;
                uint32 my_u32 = 4;
                double my_f64 = 5;
                float my_f32 = 6;
                bool my_bool = 7;
            }
            "#,
            ".TestProto",
            _file_descriptor,
            _message_descriptor
        );

        assert_de_tokens_error::<DeserializeSeedWrapper>(
            &[
                Token::Tuple { len: 6 },
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
                Token::TupleEnd,
            ],
            "Expected more protobuf message fields",
        )
    }

    #[test]
    fn deserialize_proto_wrong_field_type() {
        prepare_descriptors_and_deserializer!(
            r#"syntax = 'proto3';

            message TestProto {
                bool my_bool = 1;
            }
            "#,
            ".TestProto",
            _file_descriptor,
            _message_descriptor
        );

        assert_de_tokens_error::<DeserializeSeedWrapper>(
            &[Token::Tuple { len: 1 }, Token::Some, Token::I64(-64), Token::TupleEnd],
            "invalid type: integer `-64`, expected single Bool value",
        )
    }

    #[test]
    fn deserialize_numeric_fields_64_instead_of_32() {
        prepare_descriptors_and_deserializer!(
            r#"syntax = 'proto3';

            message TestProto {
                int32 my_i32 = 1;
                uint32 my_u32 = 2;
                float my_f32 = 3;
                TestEnum my_enum = 4;
            }

            enum TestEnum {
                A = 0;
                B = 1;
                C = 2;
            }
            "#,
            ".TestProto",
            file_descriptor,
            message_descriptor
        );

        let enum_descriptor = file_descriptor.enum_by_full_name(".TestEnum").unwrap();
        let mut expected_message = message_descriptor.new_instance();
        for (index, field) in message_descriptor.fields().enumerate() {
            let value = match index {
                0 => I32(-32),
                1 => U32(32),
                2 => F32(3.2),
                3 => Enum(enum_descriptor.clone(), 1),
                _ => unreachable!(),
            };

            field.set_singular_field(&mut *expected_message, value);
        }

        let message_deserialize = DeserializeSeedWrapper { message: expected_message };

        assert_de_tokens(
            &message_deserialize,
            &[
                Token::Tuple { len: 4 },
                Token::Some,
                Token::I64(-32),
                Token::Some,
                Token::U64(32),
                Token::Some,
                Token::F64(3.2),
                Token::Some,
                Token::I64(1),
                Token::TupleEnd,
            ],
        )
    }

    #[test]
    fn deserialize_numeric_fields_i64_instead_of_i32_with_overflow() {
        prepare_descriptors_and_deserializer!(
            r#"syntax = 'proto3';

            message TestProto {
                int32 my_i32 = 1;
            }
            "#,
            ".TestProto",
            _file_descriptor,
            _message_descriptor
        );

        assert_de_tokens_error::<DeserializeSeedWrapper>(
            &[
                Token::Tuple { len: 1 },
                Token::Some,
                Token::I64((i32::MAX as i64) + 1),
                Token::TupleEnd,
            ],
            "i32 overflow",
        )
    }

    #[test]
    fn deserialize_numeric_fields_f64_instead_of_f32_with_overflow() {
        prepare_descriptors_and_deserializer!(
            r#"syntax = 'proto3';

            message TestProto {
                float my_positive_f32 = 1;
                float my_negative_f32 = 2;
            }
            "#,
            ".TestProto",
            _file_descriptor,
            message_descriptor
        );

        let mut expected_message = message_descriptor.new_instance();
        for (index, field) in message_descriptor.fields().enumerate() {
            let value = match index {
                0 => F32(f32::INFINITY),
                1 => F32(f32::NEG_INFINITY),
                _ => unreachable!(),
            };

            field.set_singular_field(&mut *expected_message, value);
        }

        let message_deserialize = DeserializeSeedWrapper { message: expected_message };

        assert_de_tokens(
            &message_deserialize,
            &[
                Token::Tuple { len: 2 },
                Token::Some,
                Token::F64((f32::MAX as f64) * 1000.0),
                Token::Some,
                Token::F64((f32::MIN as f64) * 1000.0),
                Token::TupleEnd,
            ],
        )
    }

    #[test]
    fn deserialize_numeric_fields_unsigned_as_signed() {
        prepare_descriptors_and_deserializer!(
            r#"syntax = 'proto3';

            message TestProto {
                int32 my_i32_from_u32 = 1;
                int64 my_i64_from_u32 = 2;
                int32 my_i32_from_u64 = 3;
                int64 my_i64_from_u64 = 4;
            }
            "#,
            ".TestProto",
            _file_descriptor,
            message_descriptor
        );

        let mut expected_message = message_descriptor.new_instance();
        for (index, field) in message_descriptor.fields().enumerate() {
            let value = match index {
                0 => I32(100),
                1 => I64(100),
                2 => I32(100),
                3 => I64(100),
                _ => unreachable!(),
            };

            field.set_singular_field(&mut *expected_message, value);
        }

        let message_deserialize = DeserializeSeedWrapper { message: expected_message };

        assert_de_tokens(
            &message_deserialize,
            &[
                Token::Tuple { len: 4 },
                Token::Some,
                Token::U32(100),
                Token::Some,
                Token::U32(100),
                Token::Some,
                Token::U64(100),
                Token::Some,
                Token::U64(100),
                Token::TupleEnd,
            ],
        )
    }

    #[test]
    fn deserialize_numeric_fields_signed_as_unsigned() {
        prepare_descriptors_and_deserializer!(
            r#"syntax = 'proto3';

            message TestProto {
                uint32 my_u32_from_i32 = 1;
                uint64 my_u64_from_i32 = 2;
                uint32 my_u32_from_i64 = 3;
                uint64 my_u64_from_i64 = 4;
            }
            "#,
            ".TestProto",
            _file_descriptor,
            message_descriptor
        );

        let mut expected_message = message_descriptor.new_instance();
        for (index, field) in message_descriptor.fields().enumerate() {
            let value = match index {
                0 => U32(1000),
                1 => U64(1000),
                2 => U32(1000),
                3 => U64(1000),
                _ => unreachable!(),
            };

            field.set_singular_field(&mut *expected_message, value);
        }

        let message_deserialize = DeserializeSeedWrapper { message: expected_message };

        assert_de_tokens::<DeserializeSeedWrapper>(
            &message_deserialize,
            &[
                Token::Tuple { len: 4 },
                Token::Some,
                Token::I32(1000),
                Token::Some,
                Token::I32(1000),
                Token::Some,
                Token::I64(1000),
                Token::Some,
                Token::I64(1000),
                Token::TupleEnd,
            ],
        )
    }

    #[test]
    fn deserialize_numeric_fields_signed_as_unsigned_negative_value() {
        prepare_descriptors_and_deserializer!(
            r#"syntax = 'proto3';

            message TestProto {
                uint64 my_u_64 = 1;
            }
            "#,
            ".TestProto",
            _file_descriptor,
            _message_descriptor
        );

        assert_de_tokens_error::<DeserializeSeedWrapper>(
            &[Token::Tuple { len: 1 }, Token::Some, Token::I32(-1000), Token::TupleEnd],
            "u64 overflow",
        )
    }

    #[test]
    fn deserialize_enum_field_with_overflow() {
        prepare_descriptors_and_deserializer!(
            r#"syntax = 'proto3';

            message TestProto {
                TestEnum my_enum = 1;
            }

            enum TestEnum {
                A = 0;
                B = 1;
                C = 2;
            }
            "#,
            ".TestProto",
            _file_descriptor,
            _message_descriptor
        );

        assert_de_tokens_error::<DeserializeSeedWrapper>(
            &[
                Token::Tuple { len: 1 },
                Token::Some,
                // Enum has only 3 values defined.
                Token::I64(5),
                Token::TupleEnd,
            ],
            "enum variant index is out of bounds",
        )
    }

    #[test]
    fn deserialize_proto_with_repeated_fields() {
        prepare_descriptors_and_deserializer!(
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
            ".TestProto",
            file_descriptor,
            message_descriptor
        );

        let enum_descriptor = file_descriptor.enum_by_full_name(".TestEnum").unwrap();
        let mut expected_message = message_descriptor.new_instance();
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
                field.mut_repeated(&mut *expected_message).push(value);
            }
        }

        let message_serialize = DeserializeSeedWrapper { message: expected_message };

        assert_de_tokens(
            &message_serialize,
            &[
                Token::Tuple { len: 3 },
                Token::Seq { len: Some(0) },
                Token::SeqEnd,
                Token::Seq { len: None },
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
}
