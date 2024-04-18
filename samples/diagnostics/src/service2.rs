#![allow(missing_docs, unused_imports)]
// Copyright 2023 Google LLC

use diagnostics_api::event::Status;
use diagnostics_sample_catalog::messages::{DataItemTwo, SecondEventExt};
use sdv_middleware_extension_diagnostics::data_item::ReadOnlyDataItemHandler;
use sdv_middleware_extension_diagnostics::event::Event;
use sdv_middleware_extension_diagnostics::DiagnosticsResult;
use sdvgenerated::diagnostics_provider_service2::DiagnosticsProviderService2;
use std::sync::Arc;

struct DataItemTwoHandler {}

impl ReadOnlyDataItemHandler<DataItemTwo> for DataItemTwoHandler {
    fn read(&self) -> DiagnosticsResult<DataItemTwo> {
        Ok(DataItemTwo { array: vec![1, 2, 3], ..Default::default() })
    }
}

fn main() {
    let mut service = DiagnosticsProviderService2::new();

    let data_item_server_impl =
        service.diagnostics.create_data_item_server(Box::new(DataItemTwoHandler {}));

    service
        .sdv_diagnostics_data_item_data_item_server
        .register_service_interface(data_item_server_impl);
    service.start().unwrap();

    println!("Diagnostics provider service bundle started.");

    service
        .diagnostics
        .publish_event2_event(Event::new(
            Status::PRE_FAIL,
            Some(SecondEventExt { ..Default::default() }),
        ))
        .unwrap();

    loop {
        std::thread::park();
    }
}
