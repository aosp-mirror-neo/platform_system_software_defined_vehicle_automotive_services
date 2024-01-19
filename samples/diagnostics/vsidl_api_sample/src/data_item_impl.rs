#![allow(missing_docs)]
// Copyright 2024 Google LLC

use diagnostics_api::data_item::{ReadRequest, ReadResponse, WriteRequest};
use protobuf::well_known_types::empty::Empty;
use sdvgenerated::data_item_interface::DataItem;
use sdvmiddleware_rpc::{error::RpcError, RpcResult};
use std::sync::Mutex;

macro_rules! get_lock {
    ($mutex:expr) => {{
        match $mutex.lock() {
            Ok(x) => x,
            Err(_e) => return Err(RpcError::Other),
        }
    }};
}

#[derive(Default)]
pub struct DataItemImpl {
    id: String,
    data_item: Mutex<Vec<u8>>,
}

impl DataItemImpl {
    pub fn new(id: String) -> Self {
        DataItemImpl { id, data_item: Mutex::new(vec![1, 2, 3]) }
    }
}

impl DataItem for DataItemImpl {
    fn read(&self, request: ReadRequest) -> RpcResult<ReadResponse> {
        println!("Received read request for {:?}.", &request.id);

        if request.id == self.id {
            Ok(ReadResponse { value: get_lock!(self.data_item).clone(), ..Default::default() })
        } else {
            Err(RpcError::PermissionDenied)
        }
    }

    fn write(&self, request: WriteRequest) -> RpcResult<Empty> {
        println!("Received write request for {:?}.", &request.id);

        if request.id == self.id {
            let mut data = get_lock!(self.data_item);
            data.clear();
            data.append(&mut request.value.clone());
            drop(data);
            Ok(Empty { ..Default::default() })
        } else {
            Err(RpcError::PermissionDenied)
        }
    }
}
