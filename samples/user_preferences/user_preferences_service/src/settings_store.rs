use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use user_preferences_api::error::{Error, ResponseCode};

use crate::{settings_group::SettingsGroup, settings_group_id::SettingsGroupId};

#[derive(Clone)]
pub(crate) struct SettingsStore {
    settings_groups: Arc<Mutex<HashMap<SettingsGroupId, Arc<SettingsGroup>>>>,
}

impl SettingsStore {
    pub fn new() -> Self {
        SettingsStore { settings_groups: Arc::new(Mutex::new(HashMap::new())) }
    }

    pub fn add_group(&self, settings_group: SettingsGroup) -> Result<(), Error> {
        let mut settings_groups = self.settings_groups.lock().unwrap();

        if settings_groups.contains_key(&settings_group.id) {
            return Err(Error {
                error_code: ResponseCode::ALREADY_EXISTS.into(),
                message: format!("Settings group {} already exists", settings_group.id),
                ..Default::default()
            });
        }

        settings_groups.insert(settings_group.id.clone(), Arc::new(settings_group));

        Ok(())
    }

    pub fn get_group(&self, settings_group_id: &SettingsGroupId) -> Option<Arc<SettingsGroup>> {
        let settings_groups = self.settings_groups.lock().unwrap();

        settings_groups.get(settings_group_id).cloned()
    }

    pub fn get_all_groups(&self) -> Vec<Arc<SettingsGroup>> {
        let settings_groups = self.settings_groups.lock().unwrap();
        settings_groups.values().cloned().collect()
    }

    pub fn reset(&mut self) {
        self.settings_groups.lock().unwrap().values_mut().for_each(|group| group.reset());
    }
}
