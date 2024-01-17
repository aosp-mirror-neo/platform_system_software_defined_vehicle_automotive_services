#![allow(missing_docs)]
// Copyright 2023 Google LLC

mod data_item_impl;

use crate::data_item_impl::DataItemImpl;
use sdvgenerated::diagnostics_provider_service1::DiagnosticsProviderService1;
use std::sync::Arc;

fn main() {
    let service = DiagnosticsProviderService1::new();
    service
        .data_item_server
        .register_service_interface(Arc::new(DataItemImpl::new("data_item_id1".to_string())));

    println!("Diagnostics provider service bundle started.");

    loop {
        std::thread::park();
    }
}
