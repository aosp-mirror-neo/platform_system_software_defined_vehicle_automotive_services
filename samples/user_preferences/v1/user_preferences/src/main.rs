// Copyright 2024 Google LLC
//! Sample user preferences implementation

use std::sync::Arc;

use admin_service_impl::AdminServiceImpl;
use clap::Parser;
use log::{error, info};
use management_service_impl::ManagementServiceImpl;
use persistence::initialize_persistence;
use registry_service_impl::RegistryServiceImpl;
use rpc_client_factory::RpcClientFactoryImpl;
use rusqlite::Connection;
use sdv_comms::{id::ServiceFqin, sd::ServiceDiscoveryManager, ContextRef};
use sdv_mw_rs_sdv_comms::SdvComms;
use sdv_mw_rs_sdv_user_preferences_user_preferences_service_bundle::UserPreferencesServiceBundle;
use settings_change_propagator_impl::SettingsChangePropagatorImpl;
use std::path::PathBuf;
use subscriptions_manager_impl::SubscriptionsManagerImpl;
use tempfile::NamedTempFile;
use user_controllable_service_listener::monitor_online_services;

mod admin_service_impl;
mod management_service_impl;
mod persistence;
mod registry_service_impl;
mod rpc_client_factory;
mod settings_change_propagator_impl;
mod settings_retrieval_utils;
mod subscriptions_manager_impl;
mod user_controllable_service_listener;

static LOG_TAG: &str = "sdv_user_preferences";

#[derive(Parser, Debug)]
#[command(long_about = None)]
struct Args {
    #[arg(long, action)]
    use_temp_db: bool,
    #[arg(required_unless_present("use_temp_db"))]
    db_path: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    sdv_log::init_logger(LOG_TAG).unwrap();
    binder::ProcessState::start_thread_pool();
    info!("Starting User Preferences...");

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

    let context_ref = create_context();
    let comms = Arc::new(SdvComms { context: context_ref });
    let rpc_client_factory = Arc::new(RpcClientFactoryImpl::new(comms.clone()));
    let subscriptions_manager = Arc::new(SubscriptionsManagerImpl::new(rpc_client_factory.clone()));

    info!("Creating database at {:?}", db_path);
    let connection = Connection::open(db_path).expect("Unable to open connection to database file");
    let persistence = Arc::new(initialize_persistence(connection));

    let settings_change_propagator = Arc::new(SettingsChangePropagatorImpl::new(
        persistence.clone(),
        rpc_client_factory.clone(),
        subscriptions_manager.clone(),
    ));

    let admin_service = Arc::new(AdminServiceImpl::new(
        persistence.clone(),
        rpc_client_factory.clone(),
        settings_change_propagator.clone(),
    ));

    let management_service = Arc::new(ManagementServiceImpl::new(
        persistence.clone(),
        settings_change_propagator.clone(),
        subscriptions_manager.clone(),
    ));

    let registry_service =
        Arc::new(RegistryServiceImpl::new(persistence.clone(), settings_change_propagator.clone()));

    let _sb = UserPreferencesServiceBundle::new(
        comms,
        (admin_service.clone(), management_service.clone(), registry_service.clone()),
    )
    .await;

    let sd = Arc::new(ServiceDiscoveryManager::new(context_ref));

    monitor_online_services(sd, settings_change_propagator).await;

    info!("User Preferences started");

    binder::ProcessState::join_thread_pool();
}

fn create_context() -> ContextRef {
    let fqin = ServiceFqin::builder()
        .sdv_vm_name("vm")
        .sdv_package_name("sdv.user_preferences")
        .service_bundle_name("UserPreferencesServiceBundle")
        .service_instance_name("default")
        .build()
        .unwrap();

    ContextRef::create(fqin)
}
