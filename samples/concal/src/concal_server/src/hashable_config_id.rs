// Copyright 2024 Google LLC

use concal_api::configuration::ConfigId;
use concal_api::service_fqin::ServiceFqin;

use std::hash::Hash;
use std::string::String;

fn fqin_str(fqin: ServiceFqin) -> String {
    fqin.vm_name + &fqin.package_name + &fqin.service_name + &fqin.instance_name
}

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
        HashableConfigId {
            service_fqin: fqin_str(value.service_fqin.unwrap()),
            config_name: value.config_name,
        }
    }
}
