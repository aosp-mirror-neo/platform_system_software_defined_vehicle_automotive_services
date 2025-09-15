// Copyright 2024 Google LLC
#![allow(missing_docs)]
use log::info;
use user_preferences_api::setting::{Setting, SettingDefinition, SettingWithConstraints};

pub fn print_setting_definition(settings_definitions: &[SettingDefinition]) {
    settings_definitions.iter().for_each(|definition| {
        let setting = definition.setting_with_constraints.clone().unwrap().setting.unwrap();

        let value = option_to_string(&setting.value);

        info!("Key {:<15}, Type: {:<10?}, Value: {:<10}", setting.key, definition.kind, value)
    });

    info!("\n\n");
}

pub fn print_settings_and_constraints(setting_with_constraints: &[SettingWithConstraints]) {
    setting_with_constraints.iter().for_each(|setting_with_constraint| {
        let setting: Setting = setting_with_constraint.setting.clone().unwrap();

        let value = option_to_string(&setting.value);
        let constraints = option_to_string(&setting_with_constraint.constraints);

        info!("Key {:<15}, Value: {:<10}, Constraint: {:<10?}", setting.key, value, constraints)
    });

    info!("\n\n");
}

pub fn print_settings(setting: &[Setting]) {
    setting.iter().for_each(|setting| {
        let value = option_to_string(&setting.value);
        info!("Key {:<15} Value: {:<10}", setting.key, value)
    });

    info!("\n\n");
}

fn option_to_string<T: std::fmt::Debug>(val: &Option<T>) -> String {
    match val {
        Some(x) => format!("{x:?}"),
        None => String::from("None"),
    }
}
