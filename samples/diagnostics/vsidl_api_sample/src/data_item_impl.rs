#![allow(missing_docs)]
// Copyright 2024 Google LLC

use diagnostics_api::data_item::{ReadRequest, ReadResponse, WriteRequest};
use diagnostics_api::negative_response_code::{EmptyResponse, NegativeResponseCode::*};
use protobuf::reflect::MessageDescriptor;
use protobuf::MessageDyn;
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

pub struct DataItemImpl {
    id: String,
    descriptor: MessageDescriptor,
    data_item: Mutex<Box<dyn MessageDyn>>,
}

impl DataItemImpl {
    pub fn new(id: String, initial_data_item: Box<dyn MessageDyn>) -> Self {
        DataItemImpl {
            id,
            descriptor: initial_data_item.descriptor_dyn(),
            data_item: Mutex::new(initial_data_item),
        }
    }
}

impl DataItem for DataItemImpl {
    fn read(&self, request: ReadRequest) -> RpcResult<ReadResponse> {
        println!("Received read request for {:?}.", &request.id);

        if request.id == self.id {
            if let Ok(value) = get_lock!(self.data_item).write_length_delimited_to_bytes_dyn() {
                Ok(ReadResponse {
                    value,
                    error_code: POSITIVE_RESPONSE.into(),
                    ..Default::default()
                })
            } else {
                Ok(ReadResponse {
                    error_code: GENERAL_PROGRAMMING_FAILURE.into(),
                    ..Default::default()
                })
            }
        } else {
            Ok(ReadResponse { error_code: REQUEST_OUT_OF_RANGE.into(), ..Default::default() })
        }
    }

    fn write(&self, request: WriteRequest) -> RpcResult<EmptyResponse> {
        println!("Received write request for {:?}.", &request.id);

        if request.id == self.id {
            let mut input_stream = protobuf::CodedInputStream::from_bytes(&request.value);
            // Proto bytes are currently length delimited, ignore size varint32 at the
            // beginning
            let _skip_size = input_stream.read_raw_varint32().unwrap();

            if let Ok(new_message) = self.descriptor.parse_from(&mut input_stream) {
                let mut data = get_lock!(self.data_item);
                *data = new_message;
                Ok(EmptyResponse { error_code: POSITIVE_RESPONSE.into(), ..Default::default() })
            } else {
                Ok(EmptyResponse {
                    error_code: INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT.into(),
                    ..Default::default()
                })
            }
        } else {
            Ok(EmptyResponse { error_code: REQUEST_OUT_OF_RANGE.into(), ..Default::default() })
        }
    }
}
