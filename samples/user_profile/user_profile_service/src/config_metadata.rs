// Copyright 2024 Google LLC
use user_profile_api::user_profile_registry_service::SettingDefinition;

#[derive(Debug)]
pub struct ConfigMetadata {
    version: String,
    settings_definition: Vec<SettingDefinition>,
}

impl ConfigMetadata {
    pub fn new(version: String, settings_definition: Vec<SettingDefinition>) -> Self {
        Self { version, settings_definition }
    }

    pub fn version(&self) -> &String {
        &self.version
    }
}
