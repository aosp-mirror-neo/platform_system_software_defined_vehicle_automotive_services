// Copyright 2024 Google LLC

pub mod constraint;
pub mod error;
pub mod setting;
pub mod setting_definition;
pub mod settings_definition_repository;
pub mod settings_group_repository;
pub mod user_repository;
pub mod user_settings_repository;
pub mod vehicle_settings_repository;

use crate::persistence::settings_definition_repository::SettingDefinitionRepository;
use crate::persistence::settings_group_repository::SettingsGroupRepository;
use crate::persistence::user_repository::UserRepository;
use crate::persistence::user_settings_repository::UserSettingsRepository;
use crate::persistence::vehicle_settings_repository::VehicleSettingsRepository;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

pub struct Persistence {
    pub users: UserRepository,
    pub user_settings: UserSettingsRepository,
    pub setting_groups: SettingsGroupRepository,
    pub setting_definitions: SettingDefinitionRepository,
    pub vehicle_settings: VehicleSettingsRepository,
}

pub fn initialize_datastore(conn: Connection) -> Persistence {
    let conn = Arc::new(Mutex::new(conn));

    let users = UserRepository::new(Arc::clone(&conn))
        .expect("Unable to create an instance of user repository");

    let setting_groups = SettingsGroupRepository::new(Arc::clone(&conn))
        .expect("Unable to create an instance of settings group repository");

    let setting_definitions = SettingDefinitionRepository::new(Arc::clone(&conn))
        .expect("Unable to create an instance of settings definition repository");

    let user_settings = UserSettingsRepository::new(Arc::clone(&conn))
        .expect("Unable to create an instance of user settings repository");

    let vehicle_settings = VehicleSettingsRepository::new(Arc::clone(&conn))
        .expect("Unable to create an instance of vehicle state repository");

    Persistence { users, user_settings, setting_groups, setting_definitions, vehicle_settings }
}
