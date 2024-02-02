use concal_api::configuration::ConfigId;

use std::hash::Hash;
use std::string::String;

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct HashableConfigId {
    service_fqin: String,
    config_name: String,
}

impl HashableConfigId {
    pub fn fields(&self) -> Vec<String> {
        vec![self.service_fqin.clone(), self.config_name.clone()]
    }
    pub fn contains_empty_fields(&self) -> bool {
        self.service_fqin.is_empty() || self.config_name.is_empty()
    }
}

impl From<ConfigId> for HashableConfigId {
    fn from(value: ConfigId) -> Self {
        HashableConfigId { service_fqin: value.service_fqin, config_name: value.config_name }
    }
}
