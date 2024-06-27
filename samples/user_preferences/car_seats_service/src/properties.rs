// Copyright 2024 Google LLC

use protobuf::EnumOrUnknown;
use protobuf::MessageField;
use user_preferences_api::setting::setting::Value;
use user_preferences_api::setting::setting_and_constraints::Constraints;
use user_preferences_api::setting::{
    EnumConstraints, Setting, SettingAndConstraints, SettingDefinition, SettingKind,
};

pub(crate) const DRIVER_SEAT_SETTINGS: &str = "DRIVER_SEAT";

pub(crate) const SEAT_POSITION: &str = "SEAT_POSITION";

pub(crate) fn seat_position() -> SettingDefinition {
    SettingDefinition {
        kind: EnumOrUnknown::new(SettingKind::PER_USER),
        setting_and_constraints: MessageField::some(SettingAndConstraints {
            setting: MessageField::some(Setting {
                key: SEAT_POSITION.to_string(),
                value: Some(Value::Int64(3)),
                ..Default::default()
            }),
            constraints: Some(Constraints::EnumConstraints(EnumConstraints {
                possible_values: vec![1, 2, 3, 4, 5],
                ..Default::default()
            })),
            ..Default::default()
        }),
        ..Default::default()
    }
}
