use crate::{setting::Setting, settings_group_id::SettingsGroupId};
use itertools::Itertools;
use protobuf::MessageField;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::vec::Vec;
use user_preferences_api::error::{Error, ResponseCode};
use user_preferences_api::setting::Setting as SettingProto;
use user_preferences_api::settings_group::SettingsGroup as SettingsGroupProto;

#[derive(Clone)]
pub(crate) struct SettingsGroup {
    pub id: SettingsGroupId,
    pub version: String,
    settings: Arc<Mutex<HashMap<String, Setting>>>,
}

impl SettingsGroup {
    pub fn new(id: SettingsGroupId, version: String, settings: Vec<Setting>) -> Self {
        let group = Self { id, version, settings: Arc::new(Mutex::new(HashMap::new())) };

        for setting in settings {
            group.settings.lock().unwrap().insert(setting.name.clone(), setting);
        }

        group
    }

    pub fn reset(&self) {
        let mut settings = self.settings.lock().unwrap();
        settings.values_mut().for_each(|s| s.reset());
    }

    pub fn update(&self, new_setting_values: Vec<SettingProto>) -> Result<(), Error> {
        let mut settings = self.settings.lock().unwrap();

        if let Err(err) = Self::are_new_settings_values_valid(&settings, &new_setting_values) {
            return Err(Error {
                error_code: ResponseCode::INVALID_REQUEST.into(),
                message: err,
                ..Default::default()
            });
        }

        for new_setting in new_setting_values.into_iter() {
            let setting = settings.get_mut(&new_setting.key).unwrap();
            setting.value = new_setting.value.unwrap();
        }

        Ok(())
    }

    pub fn get(&self, setting_name: &str) -> Option<Setting> {
        let settings = self.settings.lock().unwrap();

        settings.get(setting_name).cloned()
    }

    pub fn get_all(&self) -> Vec<Setting> {
        let settings = self.settings.lock().unwrap();

        settings.values().cloned().collect_vec()
    }

    fn are_new_settings_values_valid(
        current_settings: &HashMap<String, Setting>,
        new_setting_values: &[SettingProto],
    ) -> Result<(), String> {
        for setting in new_setting_values.iter() {
            let current_settings = match current_settings.get(&setting.key) {
                Some(x) => x,
                None => {
                    return Err(format!("Setting {} does not exist within the group", setting.key))
                }
            };
            if let Some(value) = &setting.value {
                if !current_settings.is_valid_value(value) {
                    return Err(format!(
                        "{:#?} is not a valid value for Setting {}",
                        value, setting.key
                    ));
                }
            } else {
                return Err(format!("Setting {} has None value", setting.key));
            }
        }

        Ok(())
    }
}

impl From<SettingsGroup> for SettingsGroupProto {
    fn from(val: SettingsGroup) -> Self {
        let settings: Vec<SettingProto> = val.get_all().into_iter().map_into().collect_vec();
        SettingsGroupProto {
            id: MessageField::some(val.id.into()),
            version: val.version,
            settings,
            ..Default::default()
        }
    }
}
