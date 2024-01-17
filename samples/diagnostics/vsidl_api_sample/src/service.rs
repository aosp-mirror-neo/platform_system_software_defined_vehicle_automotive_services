#![allow(missing_docs)]
// Copyright 2023 Google LLC

use diagnostics_api::data_item::{ReadRequest, ReadResponse, WriteRequest};
use protobuf::well_known_types::empty::Empty;
use sdvgenerated::data_item_interface::DataItem;
use sdvgenerated::diagnostics_provider_service::DiagnosticsProviderService;
use sdvmiddleware_rpc::{error::RpcError, RpcResult};
use std::sync::Arc;

#[derive(Default)]
struct DataItemImpl {
    data_item: Vec<u8>,
}

impl DataItemImpl {
    fn new() -> Self {
        DataItemImpl { data_item: vec![1, 2, 3] }
    }
}

impl DataItem for DataItemImpl {
    fn read(&self, request: ReadRequest) -> RpcResult<ReadResponse> {
        println!("Received read request for {:?}.", &request.id);

        if request.id == "data_item_id" {
            Ok(ReadResponse { value: self.data_item.clone(), ..Default::default() })
        } else {
            Err(RpcError::PermissionDenied)
        }
    }

    fn write(&self, _request: WriteRequest) -> RpcResult<Empty> {
        Err(RpcError::PermissionDenied)
    }
}

fn main() {
    let service = DiagnosticsProviderService::new();
    service.data_item_server.register_service_interface(Arc::new(DataItemImpl::new()));

    println!("Diagnostics provider service bundle started.");

    loop {
        std::thread::park();
    }
}
