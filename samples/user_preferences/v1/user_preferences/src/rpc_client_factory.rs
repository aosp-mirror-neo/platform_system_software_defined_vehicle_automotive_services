// Copyright 2024 Google LLC

use sdv_comms::id::ServiceFqin;
use sdv_mw_rs_comms::{ClientOptions, Communicate, RpcClient};
use sdv_mw_rs_sdv_comms::SdvComms;
use sdv_status_rs::SdvResult;
use sdv_user_preferences_user_controllable_user_controllable_service_rpc::Client as UserControllableServiceClient;
use sdv_user_preferences_view_change_notifier_rpc::Client as ChangeNotifierClient;
use std::sync::Arc;

pub struct RpcClientFactoryImpl {
    comms: Arc<SdvComms>,
}

impl RpcClientFactoryImpl {
    pub fn new(comms: Arc<SdvComms>) -> Self {
        Self { comms }
    }
    pub async fn new_user_controllable_service_client(
        &self,
        fqin: &ServiceFqin,
    ) -> SdvResult<UserControllableServiceClient> {
        let attributes = sdv_mw_rs_comms::ClientAttributes {
            server_name: sdv_mw_rs_comms::UnitName {
                package: fqin.get_sdv_package_name().to_string(),
                bundle: fqin.get_service_bundle_name().to_string(),
                vm: "".to_string(),
                name: UserControllableServiceClient::DEFAULT_UNIT_NAME.to_string(),
            },
            options: ClientOptions::default(),
        };

        Ok(UserControllableServiceClient::new(
            self.comms.create_client_transport(&attributes).await?,
        ))
    }

    async fn new_change_notifier_client(
        &self,
        package: &str,
        bundle: &str,
    ) -> SdvResult<ChangeNotifierClient> {
        let attributes = sdv_mw_rs_comms::ClientAttributes {
            server_name: sdv_mw_rs_comms::UnitName {
                package: package.into(),
                bundle: bundle.into(),
                vm: "".to_string(),
                name: ChangeNotifierClient::DEFAULT_UNIT_NAME.to_string(),
            },
            options: sdv_mw_rs_comms::ClientOptions::default(),
        };
        Ok(ChangeNotifierClient::new(self.comms.create_client_transport(&attributes).await?))
    }
}
