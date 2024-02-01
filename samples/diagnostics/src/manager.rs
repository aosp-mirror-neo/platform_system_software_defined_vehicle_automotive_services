#![allow(missing_docs)]
// Copyright 2023 Google LLC

use diagnostics_api::connection::ConnectionMetadata;
use diagnostics_api::data_item::{ReadRequest, WriteRequest};
use diagnostics_api::diagnostics_manager::{
    GetConnectionMetadataRequest, GetConnectionMetadataResponse,
};
use diagnostics_api::negative_response_code::NegativeResponseCode;
use diagnostics_api::session_type::SessionType;
use protobuf::reflect::MessageDescriptor;
use protobuf::EnumOrUnknown;
use protobuf::MessageField;
use sdv_config_provider::{VsidlConfigProvider, VsidlProvider};
use sdv_uds_serde::translator::Translator;
use sdvgenerated::data_item_interface::DataItem;
use sdvgenerated::diagnostics_manager_interface::DiagnosticsManager;
use sdvgenerated::diagnostics_manager_service::{
    DiagnosticsManagerService, DiagnosticsManagerServiceCallbacks,
};
use sdvmiddleware_rpc::error::RpcError;
use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn read_data_item_request(data_item_id: &str) -> ReadRequest {
    ReadRequest { id: data_item_id.to_string(), ..Default::default() }
}

fn write_data_item_request(data_item_id: &str, value: Vec<u8>) -> WriteRequest {
    WriteRequest { id: data_item_id.to_string(), value, ..Default::default() }
}

struct DiagnosticsManagerImpl {}

impl DiagnosticsManagerImpl {
    fn new() -> Self {
        Self {}
    }
}

impl DiagnosticsManager for DiagnosticsManagerImpl {
    fn get_connection_metadata(
        &self,
        request: GetConnectionMetadataRequest,
    ) -> Result<GetConnectionMetadataResponse, RpcError> {
        Ok(match request.id.id {
            1 => GetConnectionMetadataResponse {
                metadata: MessageField::some(ConnectionMetadata {
                    source_address: 1,
                    target_address: 2,
                    session_type: SessionType::EXTENDED_DIAGNOSTIC_SESSION.into(),
                    ..Default::default()
                }),
                error_code: NegativeResponseCode::POSITIVE_RESPONSE.into(),
                ..Default::default()
            },
            2 => GetConnectionMetadataResponse {
                error_code: NegativeResponseCode::GENERAL_REJECT.into(),
                ..Default::default()
            },
            _ => {
                GetConnectionMetadataResponse {
                    // Set OEM-specific negative response code
                    error_code: EnumOrUnknown::<NegativeResponseCode>::from_i32(100),
                    ..Default::default()
                }
            }
        })
    }
}

struct DataItemInfo {
    sdv_service_name: String,
    message_descriptor: MessageDescriptor,
}

fn main() {
    let first_data_item = "data_item_id1";
    let second_data_item = "data_item_id2";

    let mut data_items = HashMap::new();
    let vsidl_provider = VsidlProvider::new().unwrap();

    for (sdv_service_name, diagnostic_declaration) in vsidl_provider.get_diagnostics_declarations()
    {
        for data_item in diagnostic_declaration.data_item {
            data_items.insert(
                data_item.id.clone(),
                DataItemInfo {
                    sdv_service_name: sdv_service_name.clone(),
                    message_descriptor: vsidl_provider
                        .get_message_descriptor(&data_item.message_name)
                        .unwrap(),
                },
            );
        }
    }

    // Create service instance and register DiagnosticsManagerServer
    let mut service = DiagnosticsManagerService::new(DiagnosticsManagerServiceCallbacks {
        event_cb: Box::new(|topic, event| {
            println!("Event {:?}, received from {:?}", event, topic);
        }),
    });
    service
        .diagnostics_manager_server
        .register_service_interface(Arc::new(DiagnosticsManagerImpl::new()));
    service.start().unwrap();
    println!("Diagnostics Manager service bundle started.");

    let first_data_item_client =
        service.get_data_item_client(&data_items[first_data_item].sdv_service_name);

    let response = first_data_item_client.read(read_data_item_request(first_data_item));
    println!("Fetch {:?}: {:?}.", first_data_item, response);

    let translator = Translator {};
    let uds_bytes_for_initial_value = translator
        .translate_from_protobuf_to_uds(
            &response.unwrap().value,
            &data_items[first_data_item].message_descriptor,
        )
        .unwrap();

    // Uds bytes are for the following proto definition:
    //
    // message DataItemOne {
    //     int32 code = 1 [(sdv.vsidl.primitive_byte_size) = ONE];
    //     string name = 2 [(sdv.vsidl.variable_type_max_size) = 5];
    // }
    //
    // fields:
    // - `code`: first byte
    // - `name`: next five bytes, with `\0` padding
    println!("Fields parsed from UDS bytes: code = '{}'", uds_bytes_for_initial_value[0]);
    let mut name_value = vec![];
    for char in uds_bytes_for_initial_value.iter().skip(1) {
        if *char != b'\0' {
            name_value.push(*char);
        }
    }
    println!("Fields parsed from UDS bytes: name = '{}'", String::from_utf8(name_value).unwrap());

    // Construct uds bytes for the following proto definition:
    //
    // message DataItemOne {
    //     int32 code = 1 [(sdv.vsidl.primitive_byte_size) = ONE];
    //     string name = 2 [(sdv.vsidl.variable_type_max_size) = 5];
    // }
    let uds_bytes_for_new_value = vec![15, b'h', b'i', 0, 0, 0];
    let new_value = translator
        .translate_from_uds_to_protobuf(
            &uds_bytes_for_new_value,
            &data_items[first_data_item].message_descriptor,
        )
        .unwrap();

    let response =
        first_data_item_client.write(write_data_item_request(first_data_item, new_value.clone()));
    println!(
        "Update {:?} to a new value {:?}. Response: {:?}.",
        first_data_item, new_value, response
    );

    let response = first_data_item_client.read(read_data_item_request(first_data_item));
    println!("Fetch {:?} data item after update: {:?}.", first_data_item, response);
    let uds_bytes_for_initial_value = translator
        .translate_from_protobuf_to_uds(
            &response.unwrap().value,
            &data_items[first_data_item].message_descriptor,
        )
        .unwrap();
    println!("Fields parsed from UDS bytes: code = '{}'", uds_bytes_for_initial_value[0]);
    let mut name_value = vec![];
    for char in uds_bytes_for_initial_value.iter().skip(1) {
        if *char != b'\0' {
            name_value.push(*char);
        }
    }
    println!("Fields parsed from UDS bytes: name = '{}'", String::from_utf8(name_value).unwrap());

    let second_data_item_client =
        service.get_data_item_client(&data_items[second_data_item].sdv_service_name);

    let response = second_data_item_client.read(read_data_item_request(first_data_item));
    println!(
        "Trying to fetch {:?} from the second provider, fails with response: {:?}.",
        first_data_item, response
    );

    let response = second_data_item_client.read(read_data_item_request(second_data_item));
    println!("Fetch {:?} from the second provider: {:?}.", second_data_item, response);

    // Sleep to wait for events from diag provider samples.
    thread::sleep(Duration::from_secs(3));

    // Print full diagnostics declaration of "DiagnosticsProviderService1" to verify that
    // all diagnostic declaration data is preserved at runtime.
    println!(
        "Full diagnostics declaration of DiagnosticsProviderService1: {}",
        vsidl_provider.get_diagnostics_declarations()["DiagnosticsProviderService1"]
    )
}
