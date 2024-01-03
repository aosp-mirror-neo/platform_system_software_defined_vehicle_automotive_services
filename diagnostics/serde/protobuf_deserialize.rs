// Copyright 2024 Google LLC
use protobuf::reflect::MessageDescriptor;
use protobuf::MessageDyn;
use serde::de::DeserializeSeed;
use serde::Deserializer;

pub(crate) struct MessageDeserializeSeed<'de> {
    _message_descriptor: &'de MessageDescriptor,
}

impl<'de> MessageDeserializeSeed<'de> {
    pub fn new(_message_descriptor: &'de MessageDescriptor) -> Self {
        MessageDeserializeSeed { _message_descriptor }
    }
}

impl<'de> DeserializeSeed<'de> for MessageDeserializeSeed<'de> {
    type Value = Box<dyn MessageDyn>;

    fn deserialize<D>(self, _: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        todo!()
    }
}
