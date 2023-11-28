use concal_api::configuration::{
    config_override_key_value::Value, ConfigId, ConfigMetadata, ConfigOverride,
};
use itertools::Itertools;
use protobuf::descriptor::FileDescriptorSet;
use protobuf::plugin::code_generator_response::File;
use protobuf::well_known_types::any::Any;
use std::collections::{BTreeMap, HashSet};
use std::fmt::Debug;
use std::string::String;
use std::vec::Vec;

#[derive(Default)]
pub struct Configuration {
    data: Any,
    descriptor: FileDescriptorSet,
    version: String,
}

impl Configuration {
    pub fn new(data: Any, descriptor: FileDescriptorSet, version: String) -> Self {
        Configuration { data, descriptor, version }
    }

    pub fn version(&self) -> &String {
        &self.version
    }

    pub fn base_config(&self) -> &Any {
        &self.data
    }

    pub fn descriptor(&self) -> &FileDescriptorSet {
        &self.descriptor
    }
}
