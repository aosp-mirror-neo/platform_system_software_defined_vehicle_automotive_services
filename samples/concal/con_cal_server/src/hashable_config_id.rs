use concal_api::configuration::{ConfigId, ConfigMetadata};

use std::hash::{Hash, Hasher};
use std::string::String;

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct HashableConfigId {
    company_domain: String,
    domain: String,
    apex: String,
    service: String,
    instance: String,
    config: String,
}

impl HashableConfigId {
    pub fn fields(&self) -> Vec<String> {
        vec![
            self.company_domain.clone(),
            self.domain.clone(),
            self.apex.clone(),
            self.service.clone(),
            self.instance.clone(),
            self.config.clone(),
        ]
    }
    pub fn contains_empty_fields(&self) -> bool {
        self.company_domain.is_empty()
            || self.domain.is_empty()
            || self.apex.is_empty()
            || self.service.is_empty()
            || self.instance.is_empty()
            || self.config.is_empty()
    }
}

impl From<ConfigId> for HashableConfigId {
    fn from(value: ConfigId) -> Self {
        HashableConfigId {
            company_domain: value.company_domain,
            domain: value.domain,
            apex: value.apex,
            service: value.service,
            instance: value.instance,
            config: value.config,
        }
    }
}
