// Copyright 2023 Google LLC
//! Vehicle and Power Mode Service.

use futures_util::StreamExt;
use log::info;
use sdv_comms::id::ServiceFqin;
use sdv_comms::ContextRef;
use sdv_mw_rs_comms::SubscribeOptions;
use sdv_mw_rs_sdv_comms::SdvComms;
use sdv_mw_rs_sdv_vpm_vehicle_client_subscriber_vehicle_state_subscriber::subscriber;
use sdv_mw_rs_sdv_vpm_vehicle_client_subscriber_vehicle_state_subscriber::VehicleStateSubscriber;
use std::sync::Arc;

static LOG_TAG: &str = "vehicle-state-subs";

#[tokio::main]
async fn main() {
    sdv_log::init_logger(LOG_TAG).unwrap();
    info!("Vehicle State subscriber is starting client starting");
    let context = create_context();
    let comms = SdvComms { context };
    let mut bundle = VehicleStateSubscriber::new(Arc::new(comms)).await.unwrap();
    let mut subscriber = bundle
        .create_observer(
            subscriber::Variant::<sdv_vpm_api::vehicle::VehicleStateChange>::UNIQUE,
            &SubscribeOptions::default(),
        )
        .await
        .unwrap();
    while let Some(Ok(vehicle_state_changes)) = subscriber.next().await {
        for vehicle_state_change in vehicle_state_changes {
            println!("Received vehicle state {:?}", vehicle_state_change)
        }
    }
}

fn create_context() -> ContextRef {
    let fqin = ServiceFqin::builder()
        .sdv_vm_name("vm")
        .sdv_package_name("sdv.vpm.vehicle.client.subscriber")
        .service_bundle_name("VehicleStateSubscriber")
        .service_instance_name("default")
        .build()
        .unwrap();

    ContextRef::create(fqin)
}
