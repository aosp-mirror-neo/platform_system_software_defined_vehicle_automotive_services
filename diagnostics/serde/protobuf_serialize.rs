// Copyright 2024 Google LLC
use protobuf::reflect::MessageDescriptor;
use protobuf::MessageDyn;
use serde::{Serialize, Serializer};

pub(crate) struct MessageSerialize<'lt> {
    _message_descriptor: &'lt MessageDescriptor,
    _message: Box<dyn MessageDyn>,
}

impl<'lt> MessageSerialize<'lt> {
    pub fn new(_message_descriptor: &'lt MessageDescriptor, _message: Box<dyn MessageDyn>) -> Self {
        Self { _message_descriptor, _message }
    }
}

impl<'lt> Serialize for MessageSerialize<'lt> {
    fn serialize<S>(&self, _: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        todo!()
    }
}
