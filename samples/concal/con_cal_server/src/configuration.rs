use concal_api::configuration::{
    config_override_key_value::Value, ConfigId, ConfigMetadata, ConfigOverride,
};
use core::clone::Clone;
use itertools::Itertools;
use protobuf::descriptor::FileDescriptorSet;
use protobuf::plugin::code_generator_response::File;
use protobuf::well_known_types::any::Any;
use std::collections::{BTreeMap, HashSet};
use std::fmt::Debug;
use std::string::String;
use std::vec::Vec;

use concal_api::configuration::config_override_key_value::Value as ConCalValue;
use protobuf::well_known_types::struct_::Value as ProtobufValue;

use crate::message_editor::MessageEditor;

#[derive(Default, Clone)]
pub struct Configuration {
    data: Any,
    descriptor: FileDescriptorSet,
    version: String,
}

impl Configuration {
    pub fn new(data: Any, descriptor: FileDescriptorSet, version: String) -> Self {
        Configuration { data, descriptor, version }
    }

    pub fn with_overrides(&self, overrides: &Vec<ConfigOverride>) -> Result<Self, String> {
        let mut message_editor = MessageEditor::new(self.data.clone(), self.descriptor.clone())?;

        for r#override in overrides {
            for kv in r#override.pairs.iter() {
                if kv.value.is_none() {
                    return Err(format!("Override contains nil value {:?}", kv));
                }
                let reflect_box_value =
                    Configuration::convert_to_reflect_box(kv.value.clone().unwrap())?;
                message_editor.update_value(&kv.key, reflect_box_value)?
            }
        }

        let derived_config = message_editor.to_any()?;

        Ok(Configuration::new(derived_config, self.descriptor.clone(), self.version.clone()))
    }

    pub fn convert_to_reflect_box(
        value: ConCalValue,
    ) -> Result<protobuf::reflect::ReflectValueBox, String> {
        match value {
            Value::ValueBool(x) => Ok(protobuf::reflect::ReflectValueBox::from(x)),
            Value::ValueInt(x) => Ok(protobuf::reflect::ReflectValueBox::from(x)),
            Value::ValueStr(x) => Ok(protobuf::reflect::ReflectValueBox::from(x)),
            Value::ValueFloat(x) => Ok(protobuf::reflect::ReflectValueBox::from(x)),
            _ => Err(format!("Cannot convert unsupported data type {:?} to reflect box", value)),
        }
    }

    pub fn version(&self) -> &String {
        &self.version
    }

    pub fn data(&self) -> &Any {
        &self.data
    }

    pub fn descriptor(&self) -> &FileDescriptorSet {
        &self.descriptor
    }
}
