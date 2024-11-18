// Copyright 2024 Google LLC
#![allow(missing_docs)]

use com_sdv_google_user_preferences_user_preferences_registry_service_rpc::Client as RegistryClient;
use sdv_comms::id::ServiceFqin;
use sdv_comms::ContextRef;
use sdv_mw_rs_com_sdv_oem_user_preferences_hvac_service::HVACService;
use sdv_mw_rs_comms::Communicate;
use sdv_mw_rs_comms::RpcClient;
use sdv_mw_rs_sdv_comms::SdvComms;
use std::sync::Arc;
use user_controllable_service_impl::{SettingGroupDefinition, SimpleUserControllableServiceImpl};

mod properties;

#[tokio::main]
async fn main() {
    let context = create_context();
    let comms = Arc::new(SdvComms { context });

    let hvac_setting_definition = SettingGroupDefinition {
        name: properties::HVAC_SETTINGS_GROUP_NAME.to_string(),
        version: String::from("1"),
        setting_definitions: vec![
            properties::hvac_enabled_definition(),
            properties::fan_speed_definition(),
            properties::temperature_definition(),
        ],
    };

    let attributes = sdv_mw_rs_comms::ClientAttributes::builder()
        .server_name(
            sdv_mw_rs_comms::UnitName::builder()
                .vm_name("")
                .package_name("com.sdv.oem.user_preferences")
                .bundle_name("UserPreferencesServiceBundle")
                .service_unit_name(RegistryClient::DEFAULT_UNIT_NAME)
                .build()
                .expect("invalid server UnitName"),
        )
        .client_options(sdv_mw_rs_comms::ClientOptions::default())
        .build()
        .expect("invalid ClientAttributes");

    let client = Arc::new(RegistryClient::new(
        comms
            .create_client_transport(&attributes)
            .await
            .expect("Failed to create client to user preferences"),
    ));

    let sb_implementation = Arc::new(
        SimpleUserControllableServiceImpl::new(vec![hvac_setting_definition], client).await,
    );

    let _hvac_service = HVACService::new(comms, (sb_implementation,))
        .await
        .expect("Failed to start Hvac Service bundle");

    println!("Started service bundle");
    std::thread::park();
}

fn create_context() -> ContextRef {
    let fqin = ServiceFqin::builder()
        .sdv_vm_name("vm")
        .sdv_package_name("com.sdv.oem.user_preferences")
        .service_bundle_name("HvacService")
        .service_instance_name("default")
        .build()
        .unwrap();

    ContextRef::create(fqin)
}
