#![allow(missing_docs)]
// Copyright 2023 Google LLC

mod data_item_impl;

use crate::data_item_impl::DataItemImpl;
use diagnostics_api::event::Status;
use diagnostics_sample_catalog::messages::{DataItemOne, FirstEventExt};
use sdv_middleware_extension_diagnostics::Event;
use sdvgenerated::diagnostics_provider_service1::DiagnosticsProviderService1;
use std::sync::Arc;

fn main() {
    let mut service = DiagnosticsProviderService1::new();
    service.data_item_server.register_service_interface(Arc::new(DataItemImpl::new(
        "data_item_id1".to_string(),
        Box::new(DataItemOne { code: 1, name: "hey".to_string(), ..Default::default() }),
    )));
    service.start().unwrap();

    println!("Diagnostics provider service bundle started.");

    service
        .diagnostics
        .publish_event1_event(Event::new(
            Status::PRE_FAIL,
            Some(FirstEventExt { ..Default::default() }),
        ))
        .unwrap();

    service.diagnostics.publish_event1_event(Event::new(Status::PRE_PASS, None)).unwrap();

    loop {
        std::thread::park();
    }
}
