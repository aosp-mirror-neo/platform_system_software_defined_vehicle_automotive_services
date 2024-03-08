#![allow(missing_docs)]
// Copyright 2023 Google LLC

mod data_item_impl;

use crate::data_item_impl::DataItemImpl;
use diagnostics_api::event::Status;
use diagnostics_sample_catalog::messages::{DataItemTwo, SecondEventExt};
use sdv_middleware_extension_diagnostics::Event;
use sdvgenerated::diagnostics_provider_service2::DiagnosticsProviderService2;
use std::sync::Arc;

fn main() {
    let mut service = DiagnosticsProviderService2::new();
    service.data_item_server.register_service_interface(Arc::new(DataItemImpl::new(
        "data_item_id2".to_string(),
        Box::new(DataItemTwo { array: vec![1, 2, 3, 4, 5], ..Default::default() }),
    )));
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
