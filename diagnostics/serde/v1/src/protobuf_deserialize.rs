// Copyright 2024 Google LLC
use crate::enum_options::EnumOptions;
use crate::field_options::FieldOptions;
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
use vsidl_v1::diagnostics::ByteSize;

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
                    if field.skip_for_diag() {
                        continue;
                    }

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

            fn visit_variable_size_seq<'de, A>(
                self,
                mut seq: A,
            ) -> Result<<Self as Visitor<'de>>::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut repeated_field = self.descriptor.mut_repeated(&mut **self.message);

                while let Some(element_value) = seq.next_element_seed(ValueDeserializeSeed::new(
                    self.descriptor,
                    &self.runtime_type,
                ))? {
                    repeated_field.push(element_value);
                }

                Ok(())
            }

            fn visit_fixed_size_seq<'de, A>(
                self,
                mut seq: A,
                count: usize,
            ) -> Result<<Self as Visitor<'de>>::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                if let Some(seq_size) = seq.size_hint() {
                    if seq_size != count {
                        return Err(A::Error::custom(format!(
                            "Fixed-size repeated field size mismatch: expected {}, actual {}",
                            count, seq_size
                        )));
                    }
                }

                let mut repeated_field = self.descriptor.mut_repeated(&mut **self.message);

                for _ in 0..count {
                    if let Some(element_value) = seq.next_element_seed(
                        ValueDeserializeSeed::new(self.descriptor, &self.runtime_type),
                    )? {
                        repeated_field.push(element_value);
                    } else {
                        return Err(A::Error::custom(format!(
                            "Fixed-size repeated field size mismatch: expected {}, actual {}",
                            count,
                            repeated_field.len()
                        )));
                    }
                }

                Ok(())
            }
        }

        impl<'de, 'lt> Visitor<'de> for RepeatedFieldVisitor<'lt> {
            type Value = ();

            fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                write!(fmt, "Repeated value")
            }

            fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                if let Some(count) =
                    self.descriptor.repeated_field_count().map_err(A::Error::custom)?
                {
                    self.visit_fixed_size_seq(seq, count)
                } else {
                    self.visit_variable_size_seq(seq)
                }
            }
        }

        match self.field_descriptor.runtime_field_type() {
            Singular(runtime_type) => {
                let field_value =
                    SingularFieldDeserializeSeed::new(self.field_descriptor, &runtime_type)
                        .deserialize(deserializer)?;

                if let Some(field_value) = field_value {
                    self.field_descriptor.set_singular_field(&mut **self.message, field_value);
                }
            }
            Repeated(runtime_type) => {
                let visitor =
                    RepeatedFieldVisitor::new(self.message, self.field_descriptor, runtime_type);

                if let Some(size) =
                    self.field_descriptor.repeated_field_count().map_err(D::Error::custom)?
                {
                    deserializer.deserialize_tuple(size, visitor)?;
                } else {
                    deserializer.deserialize_seq(visitor)?;
                }
            }
            Map(_, _) => todo!("map fields are not supported yet"),
        };
        Ok(())
    }
}

struct SingularFieldDeserializeSeed<'lt> {
    descriptor: &'lt FieldDescriptor,
    runtime_type: &'lt RuntimeType,
}

impl<'lt> SingularFieldDeserializeSeed<'lt> {
    fn new(descriptor: &'lt FieldDescriptor, runtime_type: &'lt RuntimeType) -> Self {
        Self { descriptor, runtime_type }
    }
}

impl<'de, 'lt> DeserializeSeed<'de> for SingularFieldDeserializeSeed<'lt> {
    type Value = Option<ReflectValueBox>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        /// Singular protobuf field visitor.
        ///
        /// Forwards to `ValueDeserializeSeed` if the field is present (Some(_)),
        /// or panics if it's empty (None).
        struct OptionVisitor<'lt> {
            descriptor: &'lt FieldDescriptor,
            runtime_type: &'lt RuntimeType,
        }

        impl<'lt> OptionVisitor<'lt> {
            fn new(descriptor: &'lt FieldDescriptor, runtime_type: &'lt RuntimeType) -> Self {
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
                    ValueDeserializeSeed::new(self.descriptor, self.runtime_type)
                        .deserialize(deserializer)?,
                ))
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: Error,
            {
                unimplemented!()
            }
        }

        deserializer.deserialize_option(OptionVisitor::new(self.descriptor, self.runtime_type))
    }
}

// Deserializer for individual value within the protobuf field.
//
// This may be:
// - value of the optional field
// - single element within a repeated field
// - single key or value within a map field
struct ValueDeserializeSeed<'lt> {
    options: &'lt dyn FieldOptions,
    runtime_type: &'lt RuntimeType,
}

impl<'lt> ValueDeserializeSeed<'lt> {
    fn new(options: &'lt dyn FieldOptions, runtime_type: &'lt RuntimeType) -> Self {
        Self { options, runtime_type }
    }

    fn deserialize_i32<'de, D, V>(
        primitive_byte_size: Option<ByteSize>,
        visitor: V,
        deserializer: D,
    ) -> Result<V::Value, D::Error>
    where
        D: Deserializer<'de>,
        V: Visitor<'de>,
    {
        match primitive_byte_size {
            Some(ByteSize::ONE) => deserializer.deserialize_i8(visitor),
            Some(ByteSize::TWO) => deserializer.deserialize_i16(visitor),
            _ => deserializer.deserialize_i32(visitor),
        }
    }
}

impl<'de, 'lt> DeserializeSeed<'de> for ValueDeserializeSeed<'lt> {
    type Value = ReflectValueBox;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
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

        match self.runtime_type {
            RuntimeType::I32 => Self::deserialize_i32(
                self.options.primitive_byte_size(),
                I32ValueVisitor,
                deserializer,
            ),
            RuntimeType::I64 => deserializer.deserialize_i64(I64ValueVisitor),
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

                let visitor = U32ValueVisitor;

                match self.options.primitive_byte_size() {
                    Some(ByteSize::ONE) => deserializer.deserialize_u8(visitor),
                    Some(ByteSize::TWO) => deserializer.deserialize_u16(visitor),
                    _ => deserializer.deserialize_u32(visitor),
                }
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

                if let Some((fixed_point_encoding, primitive_byte_size)) =
                    self.options.float_encoding()
                {
                    Self::deserialize_i32(primitive_byte_size, I32ValueVisitor, deserializer)
                        .and_then(|value_box| {
                            if let I32(value) = value_box {
                                Ok(F32((value as f64 * fixed_point_encoding.scale
                                    + fixed_point_encoding.offset)
                                    as f32))
                            } else {
                                Err(D::Error::custom(
                                    "Unreachable: I64ValueVisitor always emits I32",
                                ))
                            }
                        })
                } else {
                    deserializer.deserialize_f32(F32ValueVisitor)
                }
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

                if let Some(encoding) = self.options.fixed_point_encoding() {
                    deserializer.deserialize_i64(I64ValueVisitor).and_then(|value_box| {
                        if let I64(value) = value_box {
                            Ok(F64(value as f64 * encoding.scale + encoding.offset))
                        } else {
                            Err(D::Error::custom("Unreachable: I64ValueVisitor always emits I64"))
                        }
                    })
                } else {
                    deserializer.deserialize_f64(F64ValueVisitor)
                }
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

                struct FixedSizeStringValueVisitor {
                    size: usize,
                }

                impl<'de> Visitor<'de> for FixedSizeStringValueVisitor {
                    type Value = ReflectValueBox;

                    fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                        write!(fmt, "single fixed-size (size: {}) String value", self.size)
                    }

                    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
                    where
                        A: SeqAccess<'de>,
                    {
                        if let Some(seq_size) = seq.size_hint() {
                            if seq_size != self.size {
                                return Err(A::Error::custom(format!(
                                    "Fixed-size string field size mismatch: expected {}, actual {}",
                                    self.size, seq_size
                                )));
                            }
                        }

                        let mut characters = Vec::with_capacity(self.size);

                        for index in 0..self.size {
                            if let Some(char) = seq.next_element::<char>()? {
                                characters.push(char)
                            } else {
                                return Err(A::Error::custom(format!(
                                    "Fixed-size string field size mismatch: expected {}, actual {}",
                                    self.size, index
                                )));
                            }
                        }

                        Ok(String(characters.iter().collect()))
                    }
                }

                if let Some(size) = self.options.variable_type_size().map_err(D::Error::custom)? {
                    deserializer.deserialize_tuple(size, FixedSizeStringValueVisitor { size })
                } else {
                    deserializer.deserialize_string(StringValueVisitor)
                }
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

                struct FixedSizeVecU8ValueVisitor {
                    size: usize,
                }

                impl<'de> Visitor<'de> for FixedSizeVecU8ValueVisitor {
                    type Value = ReflectValueBox;

                    fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                        write!(fmt, "single variable-size Bytes value")
                    }

                    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
                    where
                        A: SeqAccess<'de>,
                    {
                        if let Some(seq_size) = seq.size_hint() {
                            if seq_size != self.size {
                                return Err(A::Error::custom(format!(
                                    "Fixed-size bytes field size mismatch: expected {}, actual {}",
                                    self.size, seq_size
                                )));
                            }
                        }

                        let mut bytes = Vec::with_capacity(self.size);

                        for index in 0..self.size {
                            if let Some(byte) = seq.next_element::<u8>()? {
                                bytes.push(byte)
                            } else {
                                return Err(A::Error::custom(format!(
                                    "Fixed-size bytes field size mismatch: expected {}, actual {}",
                                    self.size, index
                                )));
                            }
                        }

                        Ok(Bytes(bytes))
                    }
                }

                if let Some(size) = self.options.variable_type_size().map_err(D::Error::custom)? {
                    deserializer.deserialize_tuple(size, FixedSizeVecU8ValueVisitor { size })
                } else {
                    deserializer.deserialize_byte_buf(VecU8ValueVisitor)
                }
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

                let visitor = EnumValueVisitor(enum_descriptor);

                match enum_descriptor.enum_byte_size() {
                    Some(ByteSize::ONE) => deserializer.deserialize_i8(visitor),
                    Some(ByteSize::TWO) => deserializer.deserialize_i16(visitor),
                    _ => deserializer.deserialize_i32(visitor),
                }
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
    use protobuf::reflect::{
        FileDescriptor,
        ReflectValueBox::{self, *},
    };
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
    fn deserialize_proto_with_variable_type_fields() {
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
    fn deserialize_proto_with_fixed_size_variable_type_fields() {
        prepare_descriptors_and_deserializer!(
            r#"syntax = 'proto3';

            import "sdv/vsidl/v1/syntax.proto";
            import "sdv/vsidl/v1/annotations.proto";

            message TestProto {
                bytes my_bytes = 1 [(sdv.vsidl.v1.variable_type_max_size) = 2];
                bytes my_owned_bytes = 2 [(sdv.vsidl.v1.variable_type_max_size) = 4];
                bytes my_borrowed_bytes = 3 [(sdv.vsidl.v1.variable_type_max_size) = 6];
                string my_string = 4 [(sdv.vsidl.v1.variable_type_max_size) = 2];
                string my_owned_string = 5 [(sdv.vsidl.v1.variable_type_max_size) = 5];
                string my_borrowed_string = 6 [(sdv.vsidl.v1.variable_type_max_size) = 1];
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
                1 => Bytes(vec![1, 2, 3, 4]),
                2 => Bytes(vec![1, 2, 3, 4, 5, 6]),
                3 => String("hi".to_string()),
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
                Token::Tuple { len: 2 },
                Token::U8(1),
                Token::U8(2),
                Token::TupleEnd,
                Token::Some,
                Token::Tuple { len: 4 },
                Token::U8(1),
                Token::U8(2),
                Token::U8(3),
                Token::U8(4),
                Token::TupleEnd,
                Token::Some,
                Token::Tuple { len: 6 },
                Token::U8(1),
                Token::U8(2),
                Token::U8(3),
                Token::U8(4),
                Token::U8(5),
                Token::U8(6),
                Token::TupleEnd,
                Token::Some,
                Token::Tuple { len: 2 },
                Token::Char('h'),
                Token::Char('i'),
                Token::TupleEnd,
                Token::Some,
                Token::Tuple { len: 5 },
                Token::Char('w'),
                Token::Char('o'),
                Token::Char('r'),
                Token::Char('l'),
                Token::Char('d'),
                Token::TupleEnd,
                Token::Some,
                Token::Tuple { len: 1 },
                Token::Char('!'),
                Token::TupleEnd,
                Token::TupleEnd,
            ],
        )
    }

    #[test]
    fn deserialize_proto_with_fixed_size_string_field_too_short() {
        prepare_descriptors_and_deserializer!(
            r#"syntax = 'proto3';

            import "sdv/vsidl/v1/syntax.proto";
            import "sdv/vsidl/v1/annotations.proto";

            message TestProto {
                string my_string = 1 [(sdv.vsidl.v1.variable_type_max_size) = 2];
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
                Token::Tuple { len: 2 },
                Token::Char('!'),
                Token::TupleEnd,
                // Deserialization should failed by this point
                // Token::TupleEnd
            ],
            "Fixed-size string field size mismatch: expected 2, actual 1",
        )
    }

    #[test]
    #[should_panic(
        expected = "expected Token::Char('l') but deserialization wants Token::TupleEnd"
    )]
    fn deserialize_proto_with_fixed_size_string_field_too_long() {
        prepare_descriptors_and_deserializer!(
            r#"syntax = 'proto3';

            import "sdv/vsidl/v1/syntax.proto";
            import "sdv/vsidl/v1/annotations.proto";

            message TestProto {
                string my_string = 1 [(sdv.vsidl.v1.variable_type_max_size) = 2];
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
                Token::Tuple { len: 5 },
                Token::Char('h'),
                Token::Char('e'),
                Token::Char('l'),
                Token::Char('l'),
                Token::Char('o'),
                Token::TupleEnd,
                Token::TupleEnd,
            ],
            "Fixed-size string field size mismatch: expected 2, actual 5",
        )
    }

    #[test]
    fn deserialize_proto_with_fixed_size_bytes_field_too_short() {
        prepare_descriptors_and_deserializer!(
            r#"syntax = 'proto3';

            import "sdv/vsidl/v1/syntax.proto";
            import "sdv/vsidl/v1/annotations.proto";

            message TestProto {
                bytes my_bytes = 1 [(sdv.vsidl.v1.variable_type_max_size) = 2];
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
                Token::Tuple { len: 2 },
                Token::U8(1),
                Token::TupleEnd,
                // Deserialization should fail by this point
                // Token::TupleEnd,
            ],
            "Fixed-size bytes field size mismatch: expected 2, actual 1",
        )
    }

    #[test]
    #[should_panic(expected = "expected Token::U8(3) but deserialization wants Token::TupleEnd")]
    fn deserialize_proto_with_fixed_size_bytes_field_too_long() {
        prepare_descriptors_and_deserializer!(
            r#"syntax = 'proto3';

            import "sdv/vsidl/v1/syntax.proto";
            import "sdv/vsidl/v1/annotations.proto";

            message TestProto {
                bytes my_bytes = 1 [(sdv.vsidl.v1.variable_type_max_size) = 2];
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
                Token::Tuple { len: 3 },
                Token::U8(1),
                Token::U8(2),
                Token::U8(3),
                Token::TupleEnd,
                Token::TupleEnd,
            ],
            "Fixed-size bytes field size mismatch: expected 2, actual 3",
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
    #[should_panic(expected = "not implemented")]
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

        assert_de_tokens(
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

        let message_deserialize = DeserializeSeedWrapper { message: expected_message };

        assert_de_tokens(
            &message_deserialize,
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

    #[test]
    fn deserialize_proto_with_fixed_size_repeated_fields() {
        prepare_descriptors_and_deserializer!(
            r#"syntax = 'proto3';

            import "sdv/vsidl/v1/syntax.proto";
            import "sdv/vsidl/v1/annotations.proto";

            message TestProto {
                repeated int64 my_i64 = 1 [(sdv.vsidl.v1.repeated_field_max_count) = 0];
                repeated TestEnum my_enum = 2 [(sdv.vsidl.v1.repeated_field_max_count) = 2];
                repeated double my_f64 = 3  [(sdv.vsidl.v1.repeated_field_max_count) = 4];
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
                    vec![Enum(enum_descriptor.clone(), 1), Enum(enum_descriptor.clone(), 2)]
                }
                2 => {
                    vec![F64(-6.4), F64(-64.0), F64(-0.001), F64(0.0)]
                }
                _ => unreachable!(),
            };

            for value in values {
                field.mut_repeated(&mut *expected_message).push(value);
            }
        }

        let message_deserialize = DeserializeSeedWrapper { message: expected_message };

        assert_de_tokens(
            &message_deserialize,
            &[
                Token::Tuple { len: 3 },
                Token::Tuple { len: 0 },
                Token::TupleEnd,
                Token::Tuple { len: 2 },
                Token::I32(1),
                Token::I32(2),
                Token::TupleEnd,
                Token::Tuple { len: 4 },
                Token::F64(-6.4),
                Token::F64(-64.0),
                Token::F64(-0.001),
                Token::F64(0.0),
                Token::TupleEnd,
                Token::TupleEnd,
            ],
        )
    }

    #[test]
    #[should_panic(expected = "expected Token::I32(3) but deserialization wants Token::TupleEnd")]
    fn deserialize_proto_with_fixed_size_repeated_field_too_long() {
        prepare_descriptors_and_deserializer!(
            r#"syntax = 'proto3';

            import "sdv/vsidl/v1/syntax.proto";
            import "sdv/vsidl/v1/annotations.proto";

            message TestProto {
                repeated int64 my_i64 = 1 [(sdv.vsidl.v1.repeated_field_max_count) = 2];
            }
            "#,
            ".TestProto",
            _file_descriptor,
            _message_descriptor
        );

        assert_de_tokens_error::<DeserializeSeedWrapper>(
            &[
                Token::Tuple { len: 1 },
                Token::Tuple { len: 3 },
                Token::I32(1),
                Token::I32(2),
                Token::I32(3),
                Token::TupleEnd,
                Token::TupleEnd,
            ],
            "Fixed-size repeated field size mismatch: expected 2, actual 3",
        )
    }

    #[test]
    fn deserialize_proto_with_fixed_size_repeated_field_too_short() {
        prepare_descriptors_and_deserializer!(
            r#"syntax = 'proto3';

            import "sdv/vsidl/v1/syntax.proto";
            import "sdv/vsidl/v1/annotations.proto";

            message TestProto {
                repeated int64 my_i64 = 1 [(sdv.vsidl.v1.repeated_field_max_count) = 2];
            }
            "#,
            ".TestProto",
            _file_descriptor,
            _message_descriptor
        );

        assert_de_tokens_error::<DeserializeSeedWrapper>(
            &[Token::Tuple { len: 1 }, Token::Tuple { len: 1 }, Token::I64(64), Token::TupleEnd],
            "Fixed-size repeated field size mismatch: expected 2, actual 1",
        )
    }

    // Verify that variable type size checks are propagated to elements of repeated field.
    #[test]
    fn deserialize_proto_with_fixed_size_repeated_field_of_variable_type() {
        prepare_descriptors_and_deserializer!(
            r#"syntax = 'proto3';

            import "sdv/vsidl/v1/syntax.proto";
            import "sdv/vsidl/v1/annotations.proto";

            message TestProto {
                repeated string my_repeated_strings = 1
                    [(sdv.vsidl.v1.repeated_field_max_count) = 2, (sdv.vsidl.v1.variable_type_max_size) = 5];
            }
            "#,
            ".TestProto",
            _file_descriptor,
            _message_descriptor
        );

        assert_de_tokens_error::<DeserializeSeedWrapper>(
            &[
                Token::Tuple { len: 1 },
                Token::Tuple { len: 2 },
                Token::Tuple { len: 5 },
                Token::Char('h'),
                Token::Char('e'),
                Token::Char('l'),
                Token::Char('l'),
                Token::Char('o'),
                Token::TupleEnd,
                Token::Tuple { len: 2 },
                Token::Char('h'),
                Token::Char('i'),
                Token::TupleEnd,
                // Deserialization should fail by this point
                // Token::TupleEnd,
                // Token::TupleEnd,
            ],
            "Fixed-size string field size mismatch: expected 5, actual 2",
        )
    }

    #[test]
    fn deserialize_int_fields_with_byte_size_options() {
        prepare_descriptors_and_deserializer!(
            r#"syntax = 'proto3';

            import "sdv/diagnostics/v1/diagnostics.proto";

            message TestProto {
                uint32 one_byte_u32 = 1 [(sdv.diagnostics.v1.primitive_byte_size) = ONE];
                uint32 two_bytes_u32 = 2 [(sdv.diagnostics.v1.primitive_byte_size) = TWO];
                int32 one_byte_i32 = 3 [(sdv.diagnostics.v1.primitive_byte_size) = ONE];
                int32 two_bytes_i32 = 4 [(sdv.diagnostics.v1.primitive_byte_size) = TWO];
            }
            "#,
            ".TestProto",
            _file_descriptor,
            message_descriptor
        );

        let mut expected_message = message_descriptor.new_instance();
        for (index, field) in message_descriptor.fields().enumerate() {
            let value = match index {
                0 => U32(128),
                1 => U32(1024),
                2 => I32(-120),
                3 => I32(-510),
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
    fn deserialize_float_fields_with_fixed_point_options() {
        prepare_descriptors_and_deserializer!(
            r#"syntax = 'proto3';

            import "sdv/diagnostics/v1/diagnostics.proto";
            import "sdv/vsidl/v1/syntax.proto";
            import "sdv/vsidl/v1/annotations.proto";

            message TestProto {
                repeated float my_fixed_float = 1 [(sdv.diagnostics.v1.fixed_point_encoding) = {
                    scale: 0.001
                    offset: 20
                }, (sdv.vsidl.v1.repeated_field_max_count) = 2];
                repeated double my_fixed_double = 2 [(sdv.diagnostics.v1.fixed_point_encoding) = {
                    scale: 0.0001
                    offset: 0
                }, (sdv.vsidl.v1.repeated_field_max_count) = 2];
                repeated float my_one_byte_fixed_float = 3 [(sdv.diagnostics.v1.fixed_point_encoding) = {
                    scale: 0.001
                    offset: 20
                }, (sdv.vsidl.v1.repeated_field_max_count) = 2,
                (sdv.diagnostics.v1.primitive_byte_size) = ONE];
                repeated float my_two_byte_fixed_float = 4 [(sdv.diagnostics.v1.fixed_point_encoding) = {
                    scale: 0.001
                    offset: 20
                }, (sdv.vsidl.v1.repeated_field_max_count) = 2,
                (sdv.diagnostics.v1.primitive_byte_size) = TWO];
            }
            "#,
            ".TestProto",
            _file_descriptor,
            message_descriptor
        );

        let mut expected_message = message_descriptor.new_instance();
        for (index, field) in message_descriptor.fields().enumerate() {
            let values = match index {
                0 | 2 | 3 => [F32(20.013), F32(19.895)],
                1 => [F64(0.12340000000000001), F64(-0.12350000000000001)],
                _ => unreachable!(),
            };

            for value in values {
                field.mut_repeated(&mut *expected_message).push(value);
            }
        }

        let message_deserialize = DeserializeSeedWrapper { message: expected_message };

        assert_de_tokens(
            &message_deserialize,
            &[
                Token::Tuple { len: 4 },
                Token::Tuple { len: 2 },
                Token::I32(13),
                Token::I32(-105),
                Token::TupleEnd,
                Token::Tuple { len: 2 },
                Token::I64(1234),
                Token::I64(-1235),
                Token::TupleEnd,
                Token::Tuple { len: 2 },
                Token::I8(13),
                Token::I8(-105),
                Token::TupleEnd,
                Token::Tuple { len: 2 },
                Token::I16(13),
                Token::I16(-105),
                Token::TupleEnd,
                Token::TupleEnd,
            ],
        )
    }

    #[test]
    fn deserialize_proto_with_enum_with_byte_size() {
        prepare_descriptors_and_deserializer!(
            r#"syntax = 'proto3';

            import "sdv/diagnostics/v1/diagnostics.proto";

            message TestProto {
                TestEnumOne my_1_byte_enum = 1;
                TestEnumTwo my_2_byte_enum = 2;
            }

            enum TestEnumOne {
                option (sdv.diagnostics.v1.enum_byte_size) = ONE;

                A = 0;
            }

            enum TestEnumTwo {
                option (sdv.diagnostics.v1.enum_byte_size) = TWO;

                A = 0;
            }
            "#,
            ".TestProto",
            file_descriptor,
            message_descriptor
        );

        let enum_descriptor1 = file_descriptor.enum_by_full_name(".TestEnumOne").unwrap();
        let enum_descriptor2 = file_descriptor.enum_by_full_name(".TestEnumTwo").unwrap();
        let mut expected_message = message_descriptor.new_instance();
        for (index, field) in message_descriptor.fields().enumerate() {
            let value = match index {
                0 => Enum(enum_descriptor1.clone(), 0),
                1 => Enum(enum_descriptor2.clone(), 0),
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
                Token::I8(0),
                Token::Some,
                Token::I16(0),
                Token::TupleEnd,
            ],
        )
    }

    #[test]
    fn deserialize_proto_with_optional_fields() {
        prepare_descriptors_and_deserializer!(
            r#"syntax = 'proto3';

            import "sdv/vsidl/v1/syntax.proto";
            import "sdv/vsidl/v1/annotations.proto";
            import "sdv/diagnostics/v1/diagnostics.proto";

            message TestProto {
                optional int64 my_i64 = 1;
                optional InnerProto my_inner_proto = 2;
            }

            message InnerProto {
                optional uint32 my_u32 = 1;
            }
            "#,
            ".TestProto",
            file_descriptor,
            message_descriptor
        );
        let inner_descriptor = file_descriptor.message_by_full_name(".InnerProto").unwrap();

        let mut expected_message = message_descriptor.new_instance();
        for (index, field) in message_descriptor.fields().enumerate() {
            let value = match index {
                0 => I64(-64),
                1 => {
                    let mut inner_message = inner_descriptor.new_instance();
                    for (index, field) in inner_descriptor.fields().enumerate() {
                        let value = match index {
                            0 => U32(32),
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
                Token::I64(-64),
                Token::Some,
                Token::Tuple { len: 1 },
                Token::Some,
                Token::U32(32),
                Token::TupleEnd,
                Token::TupleEnd,
            ],
        )
    }

    #[test]
    fn deserialize_proto_with_skipped_fields() {
        prepare_descriptors_and_deserializer!(
            r#"syntax = 'proto3';

            import "sdv/vsidl/v1/syntax.proto";
            import "sdv/vsidl/v1/annotations.proto";
            import "sdv/diagnostics/v1/diagnostics.proto";

            message TestProto {
                int64 my_i64 = 1 [(sdv.diagnostics.v1.skip_for_diag) = true];
                int32 my_i32 = 2 [(sdv.diagnostics.v1.skip_for_diag) = true];
                uint64 my_u64 = 3 [(sdv.diagnostics.v1.skip_for_diag) = true];
                uint32 my_u32 = 4 [(sdv.diagnostics.v1.skip_for_diag) = true];
                double my_f64 = 5 [(sdv.diagnostics.v1.skip_for_diag) = true];
                float my_f32 = 6 [(sdv.diagnostics.v1.skip_for_diag) = true];
                bool my_bool = 7 [(sdv.diagnostics.v1.skip_for_diag) = true];
                InnerProto my_not_skipped_inner_proto = 8;
                InnerProto my_skipped_inner_proto = 9 [(sdv.diagnostics.v1.skip_for_diag) = true];
                repeated bool my_fixed_array = 10 [(sdv.vsidl.v1.repeated_field_max_count) = 0, (sdv.diagnostics.v1.skip_for_diag) = true];
            }

            message InnerProto {
                string my_string = 2 [(sdv.diagnostics.v1.skip_for_diag) = true];
            }
            "#,
            ".TestProto",
            file_descriptor,
            message_descriptor
        );

        let inner_descriptor = file_descriptor.message_by_full_name(".InnerProto").unwrap();
        let not_skipped_field_descriptor =
            message_descriptor.field_by_name("my_not_skipped_inner_proto").unwrap();

        let mut expected_message = message_descriptor.new_instance();
        not_skipped_field_descriptor.set_singular_field(
            &mut *expected_message,
            ReflectValueBox::Message(inner_descriptor.new_instance()),
        );

        let message_deserialize = DeserializeSeedWrapper { message: expected_message };

        assert_de_tokens(
            &message_deserialize,
            &[
                Token::Tuple { len: 1 },
                Token::Some,
                Token::Tuple { len: 0 },
                Token::TupleEnd,
                Token::TupleEnd,
            ],
        )
    }
}
