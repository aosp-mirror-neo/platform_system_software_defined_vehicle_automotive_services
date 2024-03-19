#![allow(missing_docs)]
// Copyright 2023 Google LLC

mod data_item_impl;

use crate::data_item_impl::DataItemImpl;
use diagnostics_sample_catalog::event::{event::Status, Event};
use diagnostics_sample_catalog::messages::DataItemOne;
use sdvgenerated::diagnostics_provider_service1::DiagnosticsProviderService1;
use sdvgenerated::topics::publisher::EventTopics;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn main() {
    let mut service = DiagnosticsProviderService1::new();
    service.data_item_server.register_service_interface(Arc::new(DataItemImpl::new(
        "data_item_id1".to_string(),
        Box::new(DataItemOne { code: 1, name: "hey".to_string(), ..Default::default() }),
    )));
    service.start().unwrap();

    println!("Diagnostics provider service bundle started.");

    // Sleep for 1s to ensure the correct order of logs in diagnostics manager sample.
    // I.e. event should be sent after data item communication has finished.
    thread::sleep(Duration::from_secs(1));

    service
        .event_publish(
            &EventTopics::DIAGNOSTICS_PROVIDER1_SAMPLE,
            &Event {
                id: "event1".to_string(),
                status: Status::PASS.into(),
                // omitting `extended_data` is equivalent to: `extended_data: vec![]`
                ..Default::default()
            },
        )
        .unwrap();

    service
        .event_publish(
            &EventTopics::DIAGNOSTICS_PROVIDER1_SAMPLE,
            &Event {
                id: "event1".to_string(),
                status: Status::PRE_FAIL.into(),
                extended_data: vec![],
                ..Default::default()
            },
        )
        .unwrap();

    // Check if diagnostics extension is accessible.
    let _ = service.diagnostics;

    loop {
        std::thread::park();
    }
}
