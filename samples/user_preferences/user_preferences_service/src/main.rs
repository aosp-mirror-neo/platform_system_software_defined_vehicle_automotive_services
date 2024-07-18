// Copyright 2024 Google LLC

//! # SDV User Preferences Service: Reference Implementation
//!
//! The User Preferences service provides a centralized location for storing preferences of
//! vehicle users and communicating these preferences to various components within the vehicle.

mod macros;
mod persistence;
mod user_preferences_service_impl;
mod vsidl_client_provider;

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

use crate::persistence::settings_group_repository;
use crate::persistence::user_repository;
use crate::persistence::user_settings_repository;
use crate::persistence::vehicle_settings_repository;
use crate::user_preferences_service_impl::UserPreferencesServiceImplArgs;
use crate::vsidl_client_provider::VSIDLClientProvider;
use crate::{
    persistence::settings_definition_repository, vsidl_client_provider::VSIDLClientProviderWrapper,
};

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
        user_settings_repository::UserSettingsRepository::new(Arc::clone(&connection))
            .expect("Unable to create an instance of user settings repository");

    let vehicle_settings_repository =
        vehicle_settings_repository::VehicleSettingsRepository::new(Arc::clone(&connection))
            .expect("Unable to create an instance of vehicle state repository");

    let server: Arc<UserPreferencesServer> = Arc::new(UserPreferencesServer::new());
    let vsidl_client_provider: Arc<dyn VSIDLClientProvider> =
        Arc::new(VSIDLClientProviderWrapper::new(server.clone()));

    let user_preferences_impl =
        Arc::new(UserPreferencesServiceImpl::new(UserPreferencesServiceImplArgs {
            user_repository,
            settings_group_repository,
            settings_definition_repository,
            user_settings_repository,
            vehicle_settings_repository,
            // Provide User Preferences implementation with a weak reference
            // to avoid having a circular dependency between UserPreferencesServer & UserPreferencesServiceImpl
            vsidl_client_provider: Arc::downgrade(&vsidl_client_provider),
        }));

    server
        .sdv_user_preferences_user_preferences_registry_service_server
        .register_service_interface(user_preferences_impl.clone());
    server
        .sdv_user_preferences_user_preferences_admin_service_server
        .register_service_interface(user_preferences_impl.clone());
    server
        .sdv_user_preferences_user_preferences_management_service_server
        .register_service_interface(user_preferences_impl.clone());

    info!("UserPreferencesServer service bundle started.");
    loop {
        std::thread::park();
    }
}
