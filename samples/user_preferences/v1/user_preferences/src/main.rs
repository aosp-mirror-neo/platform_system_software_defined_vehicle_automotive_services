// Copyright 2024 Google LLC
//! Sample user preferences implementation
use std::{str::FromStr, sync::Arc};

use admin_service_impl::AdminServiceImpl;
use log::error;
use log::info;
use management_service_impl::ManagementServiceImpl;
use persistence::initialize_persistence;
use registry_service_impl::RegistryServiceImpl;
use rpc_client_factory::RpcClientFactoryImpl;
use rusqlite::Connection;
use sdv_comms::sd::ServiceDiscoveryManager;
use sdv_mw_rs_com_sdv_oem_user_preferences_user_preferences_service_bundle::UserPreferencesServiceBundle;
use sdv_mw_rs_sdv_comms::SdvComms;
use settings_change_propagator_impl::SettingsChangePropagatorImpl;
use std::path::PathBuf;
use subscriptions_manager_impl::SubscriptionsManagerImpl;
use tokio_util::sync::CancellationToken;
use user_controllable_service_listener::monitor_online_services;

mod admin_service_impl;
mod management_service_impl;
mod persistence;
mod registry_service_impl;
mod rpc_client_factory;
mod rpc_client_traits;
mod settings_change_propagator_impl;
mod settings_retrieval_utils;
mod subscriptions_manager_impl;
#[cfg(test)]
mod test_utils;
mod user_controllable_service_listener;

sdv_lifecycle_client::register_service_bundle!(BundleRunner);

struct BundleRunner {
    context: ContextRef,
    cancellation_token: Option<CancellationToken>,
    runtime: Option<tokio::runtime::Runtime>,
    thread_handle: Option<tokio::task::JoinHandle<()>>,
}

impl BundleRunner {
    async fn main_loop(context: ContextRef, cancellation_token: CancellationToken) {
        info!("Starting User Preferences...");

        //TODO(b/381209295) Use the API for determining a service bundle's data directory once available
        let db_path = format!(
            "/data/service_bundle_data/{}/{}/database",
            context.get_self_fqin().get_sdv_package_name(),
            context.get_self_fqin().get_service_bundle_name(),
        );
        let db_path = match PathBuf::from_str(&db_path) {
            Ok(db_path) => db_path,
            Err(e) => {
                error!("Cannot PathBuf::from_str({db_path}): {e}");
                return;
            }
        };
        let comms = Arc::new(SdvComms { context });
        let rpc_client_factory = Arc::new(RpcClientFactoryImpl::new(comms.clone()));
        let subscriptions_manager =
            Arc::new(SubscriptionsManagerImpl::new(rpc_client_factory.clone()));

        info!("Creating database at {db_path:?}");
        let connection = match Connection::open(db_path) {
            Ok(connection) => connection,
            Err(e) => {
                error!("Unable to open connection to database file: {e}");
                return;
            }
        };
        let persistence = Arc::new(match initialize_persistence(connection) {
            Ok(persistence) => persistence,
            Err(e) => {
                error!("Initializing persistence failed: {e}");
                return;
            }
        });

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

        let registry_service = Arc::new(RegistryServiceImpl::new(
            persistence.clone(),
            settings_change_propagator.clone(),
        ));

        let _sb = UserPreferencesServiceBundle::new(
            comms,
            (admin_service.clone(), management_service.clone(), registry_service.clone()),
        )
        .await
        .inspect_err(|e| {
            error!("Error in user preferences bundle creation: {e}");
        });

        let sd = Arc::new(ServiceDiscoveryManager::new(context));

        monitor_online_services(sd, settings_change_propagator).await;

        info!("User Preferences started");

        cancellation_token.cancelled().await;
    }
}

impl ServiceBundle for BundleRunner {
    fn new(context: ContextRef) -> Self {
        binder::ProcessState::start_thread_pool();

        Self {
            context,
            thread_handle: None,
            cancellation_token: None,
            runtime: tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .inspect_err(|e| error!("Failed to build tokio runtime: {e}"))
                .ok(),
        }
    }

    fn on_start(&mut self) {
        info!("Service is starting");

        let Some(runtime) = self.runtime.as_ref() else {
            error!("Tokio runtime missing");
            return;
        };

        self.cancellation_token = Some(CancellationToken::new());
        let Some(cancellation_token_ref) = self.cancellation_token.as_ref() else {
            error!("Cancelation token creation failed");
            return;
        };
        self.thread_handle =
            Some(runtime.spawn(Self::main_loop(self.context, cancellation_token_ref.clone())));

        info!("Service is running");
    }

    fn on_stop(&mut self) {
        info!("Service is stopping");

        let Some(runtime) = self.runtime.as_ref() else {
            error!("Tokio runtime missing");
            return;
        };

        // Trigger cancellation token to exit Tokio runtime execution and drop it
        match self.cancellation_token.as_mut() {
            Some(cancellation_token) => cancellation_token.cancel(),
            None => error!("cancellation_token must have been created on_start"),
        };
        self.cancellation_token = None;

        if let Some(handle) = self.thread_handle.take() {
            runtime.block_on(async move {
                let _ = handle.await;
            });
        }

        info!("Service stopped");
    }
}
