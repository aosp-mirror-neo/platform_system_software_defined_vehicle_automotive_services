// Copyright 2024 Google LLC
use user_profile_api::configuration::ConfigId as ProtoConfigId;

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct ConfigId {
    service_fqin: String,
    config_name: String,
}

impl ConfigId {
    pub fn new(config_name: String) -> Self {
        Self {
            // Once service discovery exposes the caller identity, allow users of this class
            // to specify the service fqin
            service_fqin: String::from(""),
            config_name,
        }
    }
}

impl std::fmt::Display for ConfigId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.config_name)
    }
}
impl From<ProtoConfigId> for ConfigId {
    fn from(value: ProtoConfigId) -> Self {
        Self { service_fqin: value.service_fqdn, config_name: value.config_name }
    }
}
