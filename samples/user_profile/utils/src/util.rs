// Copyright 2024 Google LLC
#![allow(missing_docs)]
use user_profile_api::setting::{Setting, SettingDefinition};

pub fn print_setting_definition(settings_definitions: &[SettingDefinition]) {
    settings_definitions.iter().for_each(|definition| {
        let setting = definition.setting_and_constaints.clone().unwrap().setting.unwrap();

        let value = match setting.value {
            Some(x) => format!("{:?}", x),
            None => String::from("None"),
        };

        println!("Key {:<15}, Type: {:<10?}, Value {:<10}", setting.key, definition.type_, value)
    });

    println!("\n\n");
}

pub fn print_settings(setting: &[Setting]) {
    setting.iter().for_each(|setting| {
        let value = match &setting.value {
            Some(x) => format!("{:?}", x),
            None => String::from("None"),
        };

        println!("Key {:<15} Value {:<10}", setting.key, value)
    });

    println!("\n\n");
}
