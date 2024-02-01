// Copyright 2024 Google LLC
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
