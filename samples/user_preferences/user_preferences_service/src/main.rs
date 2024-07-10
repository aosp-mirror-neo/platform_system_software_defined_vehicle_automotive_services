// Copyright 2024 Google LLC
#![allow(missing_docs)]

mod macros;
mod persistence;
mod user_preferences_service_impl;

use clap::Parser;
use log::{error, info};
use rusqlite::Connection;
use sdvgenerated::user_preferences_server::UserPreferencesServer;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use user_preferences_service_impl::UserPreferencesServiceImpl;

use tempfile::NamedTempFile;

use crate::persistence::settings_definition_repository;
use crate::persistence::settings_group_repository;
use crate::persistence::user_repository;
use crate::persistence::user_settings_repository;

#[derive(Parser, Debug)]
#[command(long_about = None)]
struct Args {
    #[arg(long)]
    use_temp_db: bool,
    #[arg(required_unless_present("use_temp_db"))]
    db_path: Option<PathBuf>,
}

fn main() {
    let args: Args = Args::parse();
    let db_path = if args.use_temp_db {
        NamedTempFile::new()
            .inspect_err(|e| error!("Error creating temp file: {}", e))
            .map(|f| f.path().to_path_buf())
            .unwrap() // Panic if creating temp file fails (no recovery possible)
    } else {
        match args.db_path {
            Some(x) => x,
            None => {
                error!("Error: --db-path is required when not using --use-temp-db");
                panic!("db_path not provide");
            }
        }
    };

    let connection = Arc::new(Mutex::new(
        Connection::open(db_path).expect("Unable to open connection to database file"),
    ));
    let user_repository = user_repository::UserRepository::new(Arc::clone(&connection))
        .expect("Unable to create an instance of user repository");

    let settings_group_repository =
        settings_group_repository::SettingsGroupRepository::new(Arc::clone(&connection))
            .expect("Unable to create an instance of settings group repository");

    let settings_definition_repository =
        settings_definition_repository::SettingDefinitionRepository::new(Arc::clone(&connection))
            .expect("Unable to create an instance of settings definition repository");

    let user_settings_repository =
        user_settings_repository::UserSettingsRepository::new(connection.clone())
            .expect("Unable to create an instance of user settings repository");

    let user_preferences_impl = Arc::new(UserPreferencesServiceImpl::new(
        user_repository,
        settings_group_repository,
        settings_definition_repository,
        user_settings_repository,
    ));
    let service = UserPreferencesServer::new();

    service
        .sdv_user_preferences_user_preferences_registry_service_server
        .register_service_interface(user_preferences_impl.clone());
    service
        .sdv_user_preferences_user_preferences_admin_service_server
        .register_service_interface(user_preferences_impl.clone());
    service
        .sdv_user_preferences_user_preferences_management_service_server
        .register_service_interface(user_preferences_impl.clone());

    info!("UserPreferencesServer service bundle started.");
    loop {
        std::thread::park();
    }
}
