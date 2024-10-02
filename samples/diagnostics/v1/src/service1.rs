// Copyright 2024 Google LLC

#![allow(missing_docs)]

use diagnostics_v1_api::response_code::ResponseCode;
use diagnostics_v1_sample_catalog::messages::*;
use sdv_comms::id::ServiceFqin;
use sdv_comms::ContextRef;
use sdv_mw_rs_diag::{data_item::*, DiagnosticsResult};
use sdv_mw_rs_sdv_comms::SdvComms;
use sdv_mw_rs_sdv_sample_diagnostics_diagnostics_provider1::{
    diagnostics::Diagnostics, DiagnosticsProvider1,
};
use std::str::FromStr;
use std::sync::Arc;

use diagnostics_v1_api::event::Status;

struct PressureDataItem;
#[async_trait::async_trait]
impl ReadOnlyDataItemHandler<Pressure> for PressureDataItem {
    async fn read(&self) -> DiagnosticsResult<Pressure> {
        todo!()
    }
}

struct LowPressureThresholdDataItem;
#[async_trait::async_trait]
impl DataItemHandler<Pressure> for LowPressureThresholdDataItem {
    async fn read(&self) -> DiagnosticsResult<Pressure> {
        todo!()
    }

    async fn write(&self, _: Pressure) -> ResponseCode {
        todo!()
    }
}

#[tokio::main]
async fn main() {
    let sdv_comms = SdvComms {
        context: ContextRef::create(
            ServiceFqin::from_str("vm:sdv.diagnostics.sample.DiagnosticsProvider1/sample").unwrap(),
        ),
    };

    let service = DiagnosticsProvider1::new(
        Arc::new(sdv_comms),
        (Diagnostics::create_data_item_server(
            Arc::new(PressureDataItem {}),
            Arc::new(LowPressureThresholdDataItem {}),
        ),),
    )
    .await
    .unwrap();

    println!("Diagnostics provider service bundle started.");

    let result = service.diagnostics.lowpressure_event_publisher.publish(Status::PASS);
    println!("{result:#?}");
    let result = service.diagnostics.lowpressure_event_publisher.publish(Status::PRE_FAIL);
    println!("{result:#?}");

    loop {
        std::thread::park();
    }
}
