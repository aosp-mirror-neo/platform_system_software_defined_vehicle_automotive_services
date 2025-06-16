// Copyright 2024 Google LLC

#![allow(missing_docs)]

use sdv::comms::id::ServiceFqin;
use sdv::comms::ContextRef;
use sdv::mw::SdvComms;
use sdv_mw_rs_com_sdv_oem_sample_diagnostics_diagnostics_provider2::DiagnosticsProvider2;
use std::str::FromStr;
use std::sync::Arc;

use diagnostics_v1_sample_catalog::messages::OverheatingData;
use sdv::diagnostics::api::event::Status;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let sdv_comms = SdvComms {
        context: ContextRef::create(ServiceFqin::from_str(
            "vm:com.sdv.oem.sample.diagnostics.DiagnosticsProvider2/sample",
        )?),
    };
    let service = DiagnosticsProvider2::new(Arc::new(sdv_comms)).await?;

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
