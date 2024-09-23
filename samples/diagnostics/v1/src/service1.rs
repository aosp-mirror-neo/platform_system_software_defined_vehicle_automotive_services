// Copyright 2024 Google LLC

#![allow(missing_docs)]

use sdv_comms::id::ServiceFqin;
use sdv_comms::ContextRef;
use sdv_mw_rs_sdv_comms::SdvComms;
use sdv_mw_rs_sdv_sample_diagnostics_diagnostics_provider1::DiagnosticsProvider1;
use std::str::FromStr;
use std::sync::Arc;

use diagnostics_v1_api::event::Status;

#[tokio::main]
async fn main() {
    let sdv_comms = SdvComms {
        context: ContextRef::create(
            ServiceFqin::from_str("vm:sdv.diagnostics.sample.DiagnosticsProvider1/sample").unwrap(),
        ),
    };

    let service = DiagnosticsProvider1::new(Arc::new(sdv_comms)).await.unwrap();

    println!("Diagnostics provider service bundle started.");

    let result = service.diagnostics.lowpressure_event_publisher.publish(Status::PASS);
    println!("{result:#?}");
    let result = service.diagnostics.lowpressure_event_publisher.publish(Status::PRE_FAIL);
    println!("{result:#?}");

    loop {
        std::thread::park();
    }
}
