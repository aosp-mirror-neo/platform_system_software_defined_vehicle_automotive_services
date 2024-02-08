#![allow(missing_docs)]
// Copyright 2023 Google LLC

mod data_item_impl;

use crate::data_item_impl::DataItemImpl;
use diagnostics_sample_catalog::data_items::DataItemTwo;
use sdvgenerated::diagnostics_provider_service2::DiagnosticsProviderService2;
use std::sync::Arc;

fn main() {
    let service = DiagnosticsProviderService2::new();
    service.data_item_server.register_service_interface(Arc::new(DataItemImpl::new(
        "data_item_id2".to_string(),
        Box::new(DataItemTwo { array: vec![1, 2, 3, 4, 5], ..Default::default() }),
    )));

    println!("Diagnostics provider service bundle started.");

    loop {
        std::thread::park();
    }
}
