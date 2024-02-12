// Copyright 2023 Google LLC
//! Vehicle and Power Mode Service.

use log::{error, info};
use sdv_middleware_pubsub::Result;
use sdv_vpm_api::vehicle::VehicleStateChange;
use sdvgenerated::vehicle_state_subscriber::VehicleStateSubscriber;
use sdvgenerated::vehicle_state_subscriber::VehicleStateSubscriberCallbacks;

static LOG_TAG: &str = "vpm-sub";

fn main() {
    sdv_log::init_logger(LOG_TAG).unwrap();
    info!("Vehicle State subscriber is starting client starting");

    let mut subscriber = VehicleStateSubscriber::new(VehicleStateSubscriberCallbacks {
        vehicle_state_change_cb: Box::new(on_vehicle_state),
    });

    if let Err(err) = subscriber.start() {
        error!("Vehicle state subscriber service starting failed: {:?}", err);
        return;
    }

    loop {
        std::thread::park();
    }
}

fn on_vehicle_state(message: Result<VehicleStateChange>) {
    let vehicle_state_change = message.unwrap();
    println!("Received vehicle state {:?}", vehicle_state_change.vehicle_state)
}
