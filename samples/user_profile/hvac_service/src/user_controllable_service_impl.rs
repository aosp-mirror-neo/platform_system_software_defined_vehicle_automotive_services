use itertools::Itertools;
use protobuf::MessageField;
use sdvgenerated::user_controllable_service_interface::UserControllableService;
use user_profile_api::error::Error;
use user_profile_api::{setting::Setting, user_profile_registry_service};

use sdvgenerated::user_profile_registry_service_interface::UserProfileRegistryService;
use sdvgenerated_rpc_user_profile_registry_service::client::UserProfileRegistryServiceClient;
use sdvmiddleware_rpc::RpcResult;
use sdvmiddleware_rpc_grpc::grpc_client_transport::GrpcClientTransport;
use std::thread;

use user_profile_api::user_controllable_service::{
    FactoryResetRequest, FactoryResetResponse, RequestSettingsChangeRequest,
    RequestSettingsChangeResponse,
};

use crate::settings::{default_settings, SETTINGS, SETTINGS_GROUP_NAME};

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
        let default_settings = default_settings()
            .into_iter()
            .map(|(name, value)| Setting { key: name, value: Some(value), ..Default::default() })
            .collect_vec();

        let result = self
            .request_settings_change(RequestSettingsChangeRequest {
                group_name: SETTINGS_GROUP_NAME.to_string(),
                settings: default_settings,
                ..Default::default()
            })
            .unwrap();

        let mut response = FactoryResetResponse::new();

        if let Some(err) = result.error.into_option() {
            response.error = MessageField::some(Error {
                error_code: err.error_code,
                message: format!("Failed to perform factory reset: {}", err),
                ..Default::default()
            });
        } else {
            println!("\nPerformed factory reset");
        }
        Ok(response)
    }
}
