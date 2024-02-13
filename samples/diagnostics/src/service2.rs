#![allow(missing_docs)]
// Copyright 2023 Google LLC

mod data_item_impl;

use crate::data_item_impl::DataItemImpl;
use diagnostics_sample_catalog::data_items::DataItemTwo;
use diagnostics_sample_catalog::event::{event::Status, Event};
use sdvgenerated::diagnostics_provider_service2::DiagnosticsProviderService2;
use sdvgenerated::topics::publisher::EventTopics;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn main() {
    let mut service = DiagnosticsProviderService2::new();
    service.data_item_server.register_service_interface(Arc::new(DataItemImpl::new(
        "data_item_id2".to_string(),
        Box::new(DataItemTwo { array: vec![1, 2, 3, 4, 5], ..Default::default() }),
    )));
    service.start().unwrap();

    println!("Diagnostics provider service bundle started.");

    // Sleep for 2s to ensure the correct order of logs in diagnostics manager sample.
    // I.e. event should be sent after data item communication has finished, and the
    // event from diagnostics provider 1 has been sent.
    thread::sleep(Duration::from_secs(2));

    service
        .event_publish(
            &EventTopics::DIAGNOSTICS_PROVIDER2_SAMPLE,
            &Event {
                id: "event2".to_string(),
                status: Status::PRE_FAIL.into(),
                extended_data: vec![],
                ..Default::default()
            },
        )
        .unwrap();

    loop {
        std::thread::park();
    }
}
