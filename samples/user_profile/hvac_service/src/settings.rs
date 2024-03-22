use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::Mutex;
use user_profile_api::setting::setting::Value;

pub(crate) const ENABLED: &str = "ENABLED";
pub(crate) const SETTINGS_GROUP_NAME: &str = "HVAC";
pub(crate) const FAN_SPEED: &str = "FAN_SPEED";
pub(crate) const TEMPERATURE: &str = "TEMPERATURE";

lazy_static! {
    pub static ref SETTINGS: Mutex<HashMap<String, Value>> = Mutex::new(default_settings());
}

pub fn default_settings() -> HashMap<String, Value> {
    let mut settings: HashMap<String, Value> = HashMap::new();

    settings.insert(FAN_SPEED.to_string(), Value::Int64(1));
    settings.insert(TEMPERATURE.to_string(), Value::Int64(16));
    settings.insert(ENABLED.to_string(), Value::Bool(false));

    settings
}
