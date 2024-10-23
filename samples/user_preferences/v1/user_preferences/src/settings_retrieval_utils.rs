// Copyright 2024 Google LLC
use crate::persistence::{
    setting::{Setting, Value},
    settings_definition_repository::SettingDefinitionRepository,
    user_settings_repository::UserSettingsRepository,
    vehicle_settings_repository::VehicleSettingsRepository,
};
use sdv_status_rs::SdvResult;
use std::collections::HashMap;
use std::vec::Vec;
use user_preferences_api::setting::SettingKind;
use user_preferences_api::setting::SettingWithConstraints;

pub fn retrieve_user_settings_including_defaults_for_group(
    settings_definition_repository: &SettingDefinitionRepository,
    user_settings_repository: &UserSettingsRepository,
    user_id: i32,
    settings_group_id: i64,
) -> SdvResult<Vec<Setting>> {
    let mut user_overrides: HashMap<String, Value> = user_settings_repository
        .get_all(user_id, settings_group_id)?
        .into_iter()
        .map(|setting| (setting.name, setting.value))
        .collect();

    Ok(settings_definition_repository
        .get(settings_group_id)?
        .into_iter()
        .filter(|s| s.kind == SettingKind::PER_USER as i32)
        .map(|definition| {
            let value = user_overrides.remove(&definition.name).unwrap_or(definition.default_value);
            Setting { name: definition.name, value }
        })
        .collect())
}

pub fn retrieve_vehicle_settings_including_defaults_for_group(
    settings_definition_repository: &SettingDefinitionRepository,
    vehicle_settings_repository: &VehicleSettingsRepository,
    settings_group_id: i64,
) -> SdvResult<Vec<SettingWithConstraints>> {
    let mut applied_overrides: HashMap<String, Value> = vehicle_settings_repository
        .get(settings_group_id)?
        .into_iter()
        .map(|setting| (setting.name, setting.value))
        .collect();

    Ok(settings_definition_repository
        .get(settings_group_id)?
        .into_iter()
        .map(|definition| {
            let value =
                applied_overrides.remove(&definition.name).unwrap_or(definition.default_value);
            SettingWithConstraints {
                setting: Some(user_preferences_api::setting::Setting {
                    key: definition.name,
                    value: Some(value.into()),
                    ..Default::default()
                })
                .into(),
                constraints: definition.constraint.map(|c| c.into()),
                ..Default::default()
            }
        })
        .collect())
}
