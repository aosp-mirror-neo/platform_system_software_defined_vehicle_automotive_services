/*
 * Copyright (C) 2026 The Android Open Source Project
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use protobuf::reflect::{ProtobufValue, ReflectEqMode};
use protobuf::well_known_types::empty::Empty;
use protobuf::{Message, MessageFull};
use std::fmt::Display;

#[derive(Default, Clone, PartialEq, Debug)]
pub struct UnserializableMessage {
    value: Empty,
}

impl MessageFull for UnserializableMessage {
    fn descriptor() -> protobuf::reflect::MessageDescriptor {
        Empty::descriptor()
    }
    fn reflect_eq(&self, other: &Self, mode: &ReflectEqMode) -> bool {
        self.value.reflect_eq(&other.value, mode)
    }
}

impl ProtobufValue for UnserializableMessage {
    type RuntimeType = protobuf::reflect::rt::RuntimeTypeMessage<UnserializableMessage>;
}

impl Message for UnserializableMessage {
    const NAME: &'static str = "UnserializableMessage";

    fn is_initialized(&self) -> bool {
        false
    }

    fn merge_from(
        &mut self,
        _: &mut protobuf::CodedInputStream<'_>,
    ) -> std::result::Result<(), protobuf::Error> {
        todo!()
    }
    fn write_to_with_cached_sizes(
        &self,
        _: &mut protobuf::CodedOutputStream<'_>,
    ) -> std::result::Result<(), protobuf::Error> {
        todo!()
    }
    fn compute_size(&self) -> u64 {
        todo!()
    }
    fn special_fields(&self) -> &protobuf::SpecialFields {
        self.value.special_fields()
    }
    fn mut_special_fields(&mut self) -> &mut protobuf::SpecialFields {
        self.value.mut_special_fields()
    }
    fn new() -> Self {
        Self { value: Empty::new() }
    }
    fn default_instance() -> &'static Self {
        todo!()
    }
}

impl Display for UnserializableMessage {
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}
