use protobuf::Enum;
use std::collections::HashMap;
use user_preferences_api::user::{user::UserFlags, User};

use crate::{settings_group::SettingsGroup, settings_group_id::SettingsGroupId};

#[derive(Clone)]
pub(crate) struct VehicleConfiguration {
    pub user: User,
    pub settings_groups: HashMap<SettingsGroupId, SettingsGroup>,
}

impl VehicleConfiguration {
    pub fn new(user: User) -> Self {
        VehicleConfiguration { settings_groups: HashMap::new(), user }
    }

    pub fn is_ephemeral(&self) -> bool {
        (self.user.flags & UserFlags::EPHEMERAL.value()) == UserFlags::EPHEMERAL.value()
    }

    pub fn reset_settings(&mut self) {
        self.settings_groups
            .values_mut()
            .for_each(|group| group.settings.values_mut().for_each(|setting| setting.reset()))
    }
}
