// Copyright 2024 Google LLC

use protobuf::EnumOrUnknown;
use protobuf::MessageField;
use user_preferences_api::setting::setting::Value;
use user_preferences_api::setting::setting_and_constraints::Constraints;
use user_preferences_api::setting::{
    Int64Constraints, Setting, SettingAndConstraints, SettingDefinition, SettingKind,
};

pub(crate) const HVAC_SETTINGS_GROUP_NAME: &str = "HVAC";

pub(crate) const ENABLED: &str = "ENABLED";
pub(crate) const FAN_SPEED: &str = "FAN_SPEED";
pub(crate) const TEMPERATURE: &str = "TEMPERATURE";

pub(crate) fn hvac_enabled_definition() -> SettingDefinition {
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
    }
}

pub(crate) fn fan_speed_definition() -> SettingDefinition {
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
    }
}

pub(crate) fn temperature_definition() -> SettingDefinition {
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
    }
}
