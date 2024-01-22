#![allow(missing_docs)]
// Copyright 2024 Google LLC

use diagnostics_api::data_item::{ReadRequest, ReadResponse, WriteRequest};
use diagnostics_api::negative_response_code::{EmptyResponse, NegativeResponseCode::*};
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
            Ok(ReadResponse {
                value: get_lock!(self.data_item).clone(),
                error_code: POSITIVE_RESPONSE.into(),
                ..Default::default()
            })
        } else {
            Ok(ReadResponse { error_code: REQUEST_OUT_OF_RANGE.into(), ..Default::default() })
        }
    }

    fn write(&self, request: WriteRequest) -> RpcResult<EmptyResponse> {
        println!("Received write request for {:?}.", &request.id);

        if request.id == self.id {
            let mut data = get_lock!(self.data_item);
            data.clear();
            data.append(&mut request.value.clone());
            drop(data);
            Ok(EmptyResponse { error_code: POSITIVE_RESPONSE.into(), ..Default::default() })
        } else {
            Ok(EmptyResponse { error_code: REQUEST_OUT_OF_RANGE.into(), ..Default::default() })
        }
    }
}
