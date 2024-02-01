use std::collections::HashMap;

use user_profile_api::setting::setting::Value;

const MODE: &str = "MODE";
const FAN_SPEED: &str = "FAN_SPEED";
const TEMPERATURE: &str = "TEMPERATURE";
const ENABLED: &str = "ENABLED";

pub fn default_configuration() -> HashMap<String, Value> {
    let mut settings: HashMap<String, Value> = HashMap::new();

    settings.insert(MODE.to_string(), Value::Str(String::from("COOL")));
    settings.insert(FAN_SPEED.to_string(), Value::Int(1));
    settings.insert(TEMPERATURE.to_string(), Value::Int(16));
    settings.insert(ENABLED.to_string(), Value::Bool(false));

    settings
}
