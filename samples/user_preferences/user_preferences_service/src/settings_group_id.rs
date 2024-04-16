use user_preferences_api::settings_group::SettingsGroupId as ProtoSettingsGroupId;

use std::cmp::PartialEq;
use std::fmt;
use std::hash::{Hash, Hasher};

#[derive(Clone, Debug)]
pub(crate) struct SettingsGroupId(ProtoSettingsGroupId);

impl SettingsGroupId {
    pub fn new(service_fqin: String, name: String) -> Self {
        ProtoSettingsGroupId { service_fqin, name, ..Default::default() }.into()
    }
}

impl Hash for SettingsGroupId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.service_fqin.hash(state);
        self.0.name.hash(state);
    }
}

impl PartialEq for SettingsGroupId {
    fn eq(&self, other: &Self) -> bool {
        self.0.service_fqin == other.0.service_fqin && self.0.name == other.0.name
    }
}

impl Eq for SettingsGroupId {}

impl From<ProtoSettingsGroupId> for SettingsGroupId {
    fn from(value: ProtoSettingsGroupId) -> Self {
        SettingsGroupId(value)
    }
}

impl From<SettingsGroupId> for ProtoSettingsGroupId {
    fn from(value: SettingsGroupId) -> Self {
        value.0
    }
}

impl fmt::Display for SettingsGroupId {
    // This trait requires `fmt` with this exact signature.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Write strictly the first element into the supplied output
        // stream: `f`. Returns `fmt::Result` which indicates whether the
        // operation succeeded or failed. Note that `write!` uses syntax which
        // is very similar to `println!`.
        write!(f, "{}/{}", self.0.service_fqin, self.0.name)
    }
}
