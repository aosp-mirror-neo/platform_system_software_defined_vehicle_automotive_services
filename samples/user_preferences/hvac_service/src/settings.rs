use lazy_static::lazy_static;
use protobuf::{EnumOrUnknown, MessageField};
use std::collections::HashMap;
use std::sync::Mutex;
use user_preferences_api::setting::setting::Value;
use user_preferences_api::setting::setting_and_constraints::Constraints;
use user_preferences_api::setting::{
    Int64Constraints, Setting, SettingAndConstraints, SettingDefinition, SettingKind,
};

pub(crate) const ENABLED: &str = "ENABLED";
pub(crate) const SETTINGS_GROUP_NAME: &str = "HVAC";
pub(crate) const FAN_SPEED: &str = "FAN_SPEED";
pub(crate) const TEMPERATURE: &str = "TEMPERATURE";

lazy_static! {
    pub static ref SETTINGS: Mutex<HashMap<String, Value>> = Mutex::new(default_settings());
}

pub fn default_settings() -> HashMap<String, Value> {
    setting_definitions()
        .into_iter()
        .map(|(key, setting_definition)| {
            (
                key,
                setting_definition.setting_and_constraints.unwrap().setting.unwrap().value.unwrap(),
            )
        })
        .collect()
}

pub fn setting_definitions() -> HashMap<String, SettingDefinition> {
    let mut settings: HashMap<String, SettingDefinition> = HashMap::new();

    settings.insert(
        FAN_SPEED.to_string(),
        SettingDefinition {
            kind: EnumOrUnknown::new(SettingKind::PER_USER),
            setting_and_constraints: MessageField::some(SettingAndConstraints {
                setting: MessageField::some(Setting {
                    key: FAN_SPEED.to_string(),
                    value: Some(Value::Int64(1)),
                    ..Default::default()
                }),
                constraints: Some(Constraints::Int64Constraints(Int64Constraints {
                    min_value: Some(1),
                    max_value: Some(5),
                    ..Default::default()
                })),
                ..Default::default()
            }),
            ..Default::default()
        },
    );

    settings.insert(
        TEMPERATURE.to_string(),
        SettingDefinition {
            kind: EnumOrUnknown::new(SettingKind::PER_USER),
            setting_and_constraints: MessageField::some(SettingAndConstraints {
                setting: MessageField::some(Setting {
                    key: TEMPERATURE.to_string(),
                    value: Some(Value::Int64(16)),
                    ..Default::default()
                }),
                constraints: Some(Constraints::Int64Constraints(Int64Constraints {
                    min_value: Some(16),
                    max_value: Some(32),
                    ..Default::default()
                })),
                ..Default::default()
            }),
            ..Default::default()
        },
    );

    settings.insert(
        ENABLED.to_string(),
        SettingDefinition {
            kind: EnumOrUnknown::new(SettingKind::PER_USER),
            setting_and_constraints: MessageField::some(SettingAndConstraints {
                setting: MessageField::some(Setting {
                    key: ENABLED.to_string(),
                    value: Some(Value::Bool(false)),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
    );

    settings
}
