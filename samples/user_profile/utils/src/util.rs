// Copyright 2024 Google LLC
#![allow(missing_docs)]
use user_profile_api::setting::{Setting, SettingAndConstraints, SettingDefinition};

pub fn print_setting_definition(settings_definitions: &[SettingDefinition]) {
    settings_definitions.iter().for_each(|definition| {
        let setting = definition.setting_and_constraints.clone().unwrap().setting.unwrap();

        let value = option_to_string(&setting.value);

        println!("Key {:<15}, Type: {:<10?}, Value: {:<10}", setting.key, definition.type_, value)
    });

    println!("\n\n");
}

pub fn print_settings_and_constraints(setting_and_constraints: &[SettingAndConstraints]) {
    setting_and_constraints.iter().for_each(|setting_and_constraint| {
        let setting: Setting = setting_and_constraint.setting.clone().unwrap();

        let value = option_to_string(&setting.value);
        let constraints = option_to_string(&setting_and_constraint.constraints);

        println!("Key {:<15}, Value: {:<10}, Constraint: {:<10?}", setting.key, value, constraints)
    });

    println!("\n\n");
}

pub fn print_settings(setting: &[Setting]) {
    setting.iter().for_each(|setting| {
        let value = option_to_string(&setting.value);
        println!("Key {:<15} Value: {:<10}", setting.key, value)
    });

    println!("\n\n");
}

fn option_to_string<T: std::fmt::Debug>(val: &Option<T>) -> String {
    match val {
        Some(x) => format!("{:?}", x),
        None => String::from("None"),
    }
}
