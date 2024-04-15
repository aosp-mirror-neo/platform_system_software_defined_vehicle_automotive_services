use crate::setting::Setting;
use itertools::Itertools;
use std::collections::HashMap;
use std::vec::Vec;
use user_preferences_api::settings_group::SettingsGroup as SettingsGroupProto;

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

impl From<SettingsGroup> for SettingsGroupProto {
    fn from(val: SettingsGroup) -> Self {
        SettingsGroupProto {
            name: val.name,
            version: val.version,
            settings: val.settings.into_values().map_into().collect_vec(),
            ..Default::default()
        }
    }
}
