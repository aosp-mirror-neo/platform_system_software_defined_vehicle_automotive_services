use sdvgenerated::user_controllable_service_interface::UserControllableService;
use user_profile_api::user_profile_registry_service;

use sdvgenerated::user_profile_registry_service_interface::UserProfileRegistryService;
use sdvgenerated_rpc_user_profile_registry_service::client::UserProfileRegistryServiceClient;
use sdvmiddleware_rpc::RpcResult;
use sdvmiddleware_rpc_grpc::grpc_client_transport::GrpcClientTransport;
use std::thread;

use user_profile_api::user_controllable_service::{
    FactoryResetRequest, FactoryResetResponse, RequestSettingsChangeRequest,
    RequestSettingsChangeResponse,
};

use crate::settings::{default_settings, SETTINGS};

pub(crate) struct UserControllableServiceImpl {}

impl UserControllableServiceImpl {
    pub fn new() -> Self {
        Self {}
    }

    fn client() -> UserProfileRegistryServiceClient {
        UserProfileRegistryServiceClient::new(Box::new(GrpcClientTransport::new(
            "UserProfileServer",
        )))
    }
}

impl UserControllableService for UserControllableServiceImpl {
    fn request_settings_change(
        &self,
        request: RequestSettingsChangeRequest,
    ) -> RpcResult<RequestSettingsChangeResponse> {
        // Typically services controlling hardware components take some time to reconfigure a device, to simulate the time delay
        // we spawn a thread that notifies User Profile asynchronously of the setting changes after a couple of seconds
        thread::spawn(move || {
            let mut settings = SETTINGS.lock().unwrap();

            println!("Settings change request, simulating setting change delay\n\n");
            thread::sleep(std::time::Duration::from_secs(2));

            request.settings.iter().for_each(|setting| {
                settings.insert(setting.key.clone(), setting.value.clone().unwrap());
            });

            println!(
                "Applied changes from request with transaction ID: {}",
                request.transaction_id
            );

            println!("Current settings");
            settings
                .iter()
                .for_each(|(key, value)| println!("Key {:<15} Value {:<10?}", key, value));

            let client = Self::client();
            let response = client
                .update_settings(user_profile_registry_service::UpdateSettingsRequest {
                    group_name: request.group_name,
                    transaction_id: request.transaction_id,
                    settings: request.settings,
                    ..Default::default()
                })
                .expect("Failed to perform GRPC to update settings");
            if response.error.is_some() {
                panic!("Failed update settings within user profile {:#?}", response.error.unwrap());
            }
        });

        Ok(RequestSettingsChangeResponse::new())
    }

    fn factory_reset(&self, _request: FactoryResetRequest) -> RpcResult<FactoryResetResponse> {
        let mut settings = SETTINGS.lock().unwrap();

        *settings = default_settings();

        println!("\nPerformed factory reset");
        settings.iter().for_each(|(key, value)| println!("Key {:<15} Value {:<10?}", key, value));

        Ok(FactoryResetResponse::new())
    }
}
