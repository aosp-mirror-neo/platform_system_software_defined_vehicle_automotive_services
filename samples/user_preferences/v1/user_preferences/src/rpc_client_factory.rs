// Copyright 2024 Google LLC

use async_trait::async_trait;
use com_sdv_google_user_preferences_user_controllable_user_controllable_service_rpc::Client as UserControllableServiceClient;
use com_sdv_google_user_preferences_view_change_notifier_rpc::Client as ChangeNotifierClient;
use mockall::automock;
use sdv_comms::id::ServiceFqin;
use sdv_mw_rs_comms::{ClientOptions, Communicate, RpcClient};
use sdv_mw_rs_sdv_comms::SdvComms;
use sdv_status_rs::SdvResult;
use std::sync::Arc;

use crate::rpc_client_traits::{ChangeNotifierClientTrait, UserControllableServiceClientTrait};

#[automock]
#[async_trait]
pub trait RpcClientFactory: Send + Sync {
    async fn new_user_controllable_service_client(
        &self,
        fqin: &ServiceFqin,
    ) -> SdvResult<Box<dyn UserControllableServiceClientTrait>>;

    async fn new_change_notifier_client(
        &self,
        package: &str,
        bundle: &str,
    ) -> SdvResult<Box<dyn ChangeNotifierClientTrait>>;
}

pub struct RpcClientFactoryImpl {
    comms: Arc<SdvComms>,
}

impl RpcClientFactoryImpl {
    pub fn new(comms: Arc<SdvComms>) -> Self {
        Self { comms }
    }
}

#[async_trait]
impl RpcClientFactory for RpcClientFactoryImpl {
    async fn new_user_controllable_service_client(
        &self,
        fqin: &ServiceFqin,
    ) -> SdvResult<Box<dyn UserControllableServiceClientTrait>> {
        let attributes = sdv_mw_rs_comms::ClientAttributes::builder()
            .server_name(
                sdv_mw_rs_comms::UnitName::builder()
                    .vm_name("")
                    .package_name(fqin.get_sdv_package_name())
                    .bundle_name(fqin.get_service_bundle_name())
                    .service_unit_name(UserControllableServiceClient::DEFAULT_UNIT_NAME)
                    .build()?,
            )
            .client_options(ClientOptions::default())
            .build()?;
        Ok(Box::new(UserControllableServiceClient::new(
            self.comms.create_client_transport(&attributes).await?,
        )))
    }

    async fn new_change_notifier_client(
        &self,
        package: &str,
        bundle: &str,
    ) -> SdvResult<Box<dyn ChangeNotifierClientTrait>> {
        let attributes = sdv_mw_rs_comms::ClientAttributes::builder()
            .server_name(
                sdv_mw_rs_comms::UnitName::builder()
                    .vm_name("")
                    .package_name(package)
                    .bundle_name(bundle)
                    .service_unit_name(ChangeNotifierClient::DEFAULT_UNIT_NAME)
                    .build()?,
            )
            .client_options(ClientOptions::default())
            .build()?;
        Ok(Box::new(ChangeNotifierClient::new(
            self.comms.create_client_transport(&attributes).await?,
        )))
    }
}
