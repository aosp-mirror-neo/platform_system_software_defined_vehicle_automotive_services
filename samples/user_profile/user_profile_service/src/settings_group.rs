use crate::setting::Setting;
use std::collections::HashMap;
use std::vec::Vec;

#[derive(Clone)]
pub(crate) struct SettingsGroup {
    pub name: String,
    pub version: String,
    pub settings: HashMap<String, Setting>,
}

impl SettingsGroup {
    pub fn new(name: String, version: String, settings: Vec<Setting>) -> Self {
        let mut group = Self { name, version, settings: HashMap::new() };

        for setting in settings {
            group.settings.insert(setting.name.clone(), setting);
        }

        group
    }
}
