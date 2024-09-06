// Copyright 2024 Google LLC

use mockall::{automock, mock};
use sdvgenerated::user_preferences_server::UserPreferencesServer;
use sdvgenerated_rpc_sdv_user_preferences_user_controllable_user_controllable_service::client::UserControllableServiceClient;
use sdvgenerated_rpc_sdv_user_preferences_view_change_notifier::client::ChangeNotifierClient;
use sdvmiddleware_rpc::{
    client_transport::ClientTransport as MiddlewareClientTransport, RpcResult,
};
use std::sync::Arc;

#[automock]
pub trait VSIDLClientProvider: Send + Sync {
    fn get_sdv_user_preferences_view_change_notifier_client(
        &self,
        target_service_bundle: &str,
    ) -> Arc<ChangeNotifierClient>;

    fn get_sdv_user_preferences_user_controllable_user_controllable_service_client(
        &self,
        target_service_bundle: &str,
    ) -> Arc<UserControllableServiceClient>;
}

pub struct VSIDLClientProviderWrapper {
    server: Arc<UserPreferencesServer>,
}

impl VSIDLClientProviderWrapper {
    pub fn new(server: Arc<UserPreferencesServer>) -> Self {
        VSIDLClientProviderWrapper { server }
    }
}

impl VSIDLClientProvider for VSIDLClientProviderWrapper {
    fn get_sdv_user_preferences_view_change_notifier_client(
        &self,
        target_service_bundle: &str,
    ) -> Arc<ChangeNotifierClient> {
        self.server.get_sdv_user_preferences_view_change_notifier_client(target_service_bundle)
    }

    fn get_sdv_user_preferences_user_controllable_user_controllable_service_client(
        &self,
        target_service_bundle: &str,
    ) -> Arc<UserControllableServiceClient> {
        self.server.get_sdv_user_preferences_user_controllable_user_controllable_service_client(
            target_service_bundle,
        )
    }
}

mock! {
    pub ClientTransport {}
    impl MiddlewareClientTransport for ClientTransport {
        fn call_method(&self, method: &str, request: &[u8]) -> RpcResult<Vec<u8>>;
    }
}
