use std::collections::HashMap;

use protobuf::Enum;
use user_profile_api::user::{user::UserFlags, User};

use crate::settings_group::SettingsGroup;

pub(crate) struct VehicleConfiguration {
    pub user: User,
    pub settings_groups: HashMap<String, SettingsGroup>,
}

impl VehicleConfiguration {
    pub fn new(user: User) -> Self {
        VehicleConfiguration { settings_groups: HashMap::new(), user }
    }

    pub fn is_ephemeral(&self) -> bool {
        (self.user.flags & UserFlags::EPHEMERAL.value()) == UserFlags::EPHEMERAL.value()
    }
}
