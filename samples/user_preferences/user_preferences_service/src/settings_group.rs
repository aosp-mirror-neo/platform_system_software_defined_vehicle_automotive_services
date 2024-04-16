use crate::{setting::Setting, settings_group_id::SettingsGroupId};
use itertools::Itertools;
use protobuf::MessageField;
use std::collections::HashMap;
use std::vec::Vec;
use user_preferences_api::settings_group::SettingsGroup as SettingsGroupProto;

#[derive(Clone)]
pub(crate) struct SettingsGroup {
    pub id: SettingsGroupId,
    pub version: String,
    pub settings: HashMap<String, Setting>,
}

impl SettingsGroup {
    pub fn new(id: SettingsGroupId, version: String, settings: Vec<Setting>) -> Self {
        let mut group = Self { id, version, settings: HashMap::new() };

        for setting in settings {
            group.settings.insert(setting.name.clone(), setting);
        }

        group
    }
}

impl From<SettingsGroup> for SettingsGroupProto {
    fn from(val: SettingsGroup) -> Self {
        SettingsGroupProto {
            id: MessageField::some(val.id.into()),
            version: val.version,
            settings: val.settings.into_values().map_into().collect_vec(),
            ..Default::default()
        }
    }
}
