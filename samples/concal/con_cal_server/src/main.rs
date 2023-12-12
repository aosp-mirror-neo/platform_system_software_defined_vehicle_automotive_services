// Copyright 2023 Google LLC
#![allow(missing_docs)]

#[cfg(test)]
mod tests;

mod con_cal_service_impl;
mod configuration;
mod hashable_config_id;
mod message_editor;
mod trie;
mod versioned_object;

use std::sync::Arc;

use con_cal_service_impl::ConCalServiceImpl;
use sdvgenerated::con_cal_server::ConCalServer;

fn main() {
    let service = ConCalServer::new();
    service.con_cal_service_server.register_service_interface(Arc::new(ConCalServiceImpl::new()));

    println!("ConCalServer service bundle started.");
    loop {
        std::thread::park();
    }
}
