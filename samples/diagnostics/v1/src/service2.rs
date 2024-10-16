// Copyright 2024 Google LLC

#![allow(missing_docs)]

use sdv::comms::id::ServiceFqin;
use sdv::comms::ContextRef;
use sdv::mw::SdvComms;
use sdv_mw_rs_sdv_sample_diagnostics_diagnostics_provider2::DiagnosticsProvider2;
use std::str::FromStr;
use std::sync::Arc;

use diagnostics_v1_api::event::Status;
use diagnostics_v1_sample_catalog::messages::OverheatingData;

#[tokio::main]
async fn main() {
    let sdv_comms = SdvComms {
        context: ContextRef::create(
            ServiceFqin::from_str("vm:sdv.diagnostics.sample.DiagnosticsProvider2/sample").unwrap(),
        ),
    };
    let service = DiagnosticsProvider2::new(Arc::new(sdv_comms)).await.unwrap();

    println!("Diagnostics provider service bundle started.");

    let result = service.diagnostics.overheating_event_publisher.publish(Status::PASS);
    println!("{result:#?}");

    let result = service
        .diagnostics
        .overheating_event_publisher
        .publish_with_data(Status::PASS, OverheatingData::default());
    println!("{result:#?}");

    loop {
        std::thread::park();
    }
}
