// Copyright 2024 Google LLC
use user_profile_api::setting::SettingDefinition;

#[derive(Debug)]
pub struct SettingsGroupMetadata {
    version: String,
    settings_definition: Vec<SettingDefinition>,
}

impl SettingsGroupMetadata {
    pub fn new(version: String, settings_definition: Vec<SettingDefinition>) -> Self {
        Self { version, settings_definition }
    }

    pub fn version(&self) -> &String {
        &self.version
    }

    pub fn settings_definition(&self) -> &Vec<SettingDefinition> {
        &self.settings_definition
    }
}
