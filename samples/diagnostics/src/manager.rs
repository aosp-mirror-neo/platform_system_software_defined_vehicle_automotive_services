#![allow(missing_docs)]
// Copyright 2023 Google LLC

use diagnostics_api::connection::ConnectionMetadata;
use diagnostics_api::data_item::{ReadRequest, WriteRequest};
use diagnostics_api::diagnostics_manager::{
    GetConnectionMetadataRequest, GetConnectionMetadataResponse,
};
use diagnostics_api::event::Event;
use diagnostics_api::negative_response_code::NegativeResponseCode;
use diagnostics_api::routine_control::RoutineControlRequest;
use diagnostics_api::session_type::SessionType;
use protobuf::EnumOrUnknown;
use protobuf::MessageField;
use sdv_config_provider::{VsidlConfigProvider, VsidlProvider};
use sdv_middleware_pubsub::subscriber::DataTunnelSubscriber;
use sdv_uds_serde::diagnostics_translator::DiagnosticsTranslator;
use sdv_uds_serde::entity_id::{EntityId, MethodMessage, RoutineMethod};
use sdvgenerated::data_item_interface::DataItem;
use sdvgenerated::diagnostics_manager_interface::DiagnosticsManager;
use sdvgenerated::diagnostics_manager_service::DiagnosticsManagerService;
use sdvgenerated::routine_control_interface::RoutineControl;
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

#[derive(Clone, Eq, Hash, PartialEq, Debug)]
pub enum EventTopics {
    Event1,
    Event2,
}

impl sdv_middleware_pubsub::EnumTopic for EventTopics {
    /// Returns iterator over all available topics
    fn iterator() -> std::slice::Iter<'static, Self> {
        static TOPICS: [EventTopics; 2] = [EventTopics::Event1, EventTopics::Event2];
        TOPICS.iter()
    }

    /// Returns string name of the topic
    fn get_name(&self) -> &str {
        match self {
            EventTopics::Event1 => "diagnostics_provider_service1.event.event1",
            EventTopics::Event2 => "diagnostics_provider_service2.event.event2",
        }
    }
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

fn main() {
    let first_data_item = "data_item_id1";
    let second_data_item = "data_item_id2";

    // Mapping from OEM config:
    let mut data_items = HashMap::new();

    data_items.insert(first_data_item.to_string(), "DiagnosticsProviderService1".to_string());

    data_items.insert(second_data_item.to_string(), "DiagnosticsProviderService2".to_string());

    // Create service instance and register DiagnosticsManagerServer
    let mut service = DiagnosticsManagerService::new();
    service
        .diagnostics_manager_server
        .register_service_interface(Arc::new(DiagnosticsManagerImpl::new()));
    service.start().unwrap();

    // Sleep to wait for diagnostic providers to register.
    thread::sleep(Duration::from_secs(2));

    println!("Diagnostics Manager service bundle started.");

    let first_data_item_client = service.get_data_item_client(&data_items[first_data_item]);

    let response = first_data_item_client.read(read_data_item_request(first_data_item));
    println!("Fetch {:?}: {:?}.", first_data_item, response);

    let translator = DiagnosticsTranslator::new();
    let uds_bytes_for_initial_value = translator
        .translate_from_protobuf_to_uds(
            &data_items[first_data_item],
            EntityId::DataItem(first_data_item.to_string()),
            &response.unwrap().value,
        )
        .unwrap();

    // Uds bytes are for the following proto definition:
    //
    // message DataItemOne {
    //   int32 code = 1 [(sdv.vsidl.primitive_byte_size) = ONE];
    //   bool enabled = 2;
    //   string name = 3 [(sdv.vsidl.variable_type_max_size) = 5];
    // }
    //
    // fields:
    // - `code`: first byte
    // - `name`: next five bytes, with `\0` padding
    println!("Fields parsed from UDS bytes: code = '{}'", uds_bytes_for_initial_value[0]);
    let mut name_value = vec![];
    for char in uds_bytes_for_initial_value.iter().skip(2) {
        if *char != b'\0' {
            name_value.push(*char);
        }
    }
    println!("Fields parsed from UDS bytes: name = '{}'", String::from_utf8(name_value).unwrap());

    // Construct uds bytes for the following proto definition:
    //
    // message DataItemOne {
    //   int32 code = 1 [(sdv.vsidl.primitive_byte_size) = ONE];
    //   bool enabled = 2;
    //   string name = 3 [(sdv.vsidl.variable_type_max_size) = 5];
    // }
    let uds_bytes_for_new_value = vec![15, 0, b'h', b'i', 0, 0, 0];
    let new_value = translator
        .translate_from_uds_to_protobuf(
            &data_items[first_data_item],
            EntityId::DataItem(first_data_item.to_string()),
            &uds_bytes_for_new_value,
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
            &data_items[first_data_item],
            EntityId::DataItem(first_data_item.to_string()),
            &response.unwrap().value,
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

    let second_data_item_client = service.get_data_item_client(&data_items[second_data_item]);

    let response = second_data_item_client.read(read_data_item_request(first_data_item));
    println!(
        "Trying to fetch {:?} from the second provider, fails with response: {:?}.",
        first_data_item, response
    );

    let response = second_data_item_client.read(read_data_item_request(second_data_item));
    println!("Fetch {:?} from the second provider: {:?}.", second_data_item, response);

    let response =
        second_data_item_client.write(write_data_item_request(second_data_item, new_value.clone()));
    println!(
        "Trying to write {:?} to the second provider, fails with response: {:?}.",
        second_data_item, response
    );

    // Diagnostics routines communication:
    let routine_client = service.get_routine_control_client("DiagnosticsProviderService1");

    println!(
        "Starting routine: {:?}",
        routine_client.start(RoutineControlRequest {
            id: "routine1".to_string(),
            request: translator
                .translate_from_uds_to_protobuf(
                    &data_items[first_data_item],
                    EntityId::Routine {
                        id: "routine1".to_string(),
                        method: RoutineMethod::Start,
                        message: MethodMessage::Request
                    },
                    &[b's', b't', b'a', b'r', b't', 0, 0, 0, 0, 0],
                )
                .unwrap(),
            ..Default::default()
        })
    );
    println!(
        "Stopping routine: {:?}",
        routine_client.stop(RoutineControlRequest {
            id: "routine1".to_string(),
            request: translator
                .translate_from_uds_to_protobuf(
                    &data_items[first_data_item],
                    EntityId::Routine {
                        id: "routine1".to_string(),
                        method: RoutineMethod::Stop,
                        message: MethodMessage::Request
                    },
                    &[b's', b't', b'o', b'p', 0, 0, 0, 0, 0, 0],
                )
                .unwrap(),
            ..Default::default()
        })
    );
    println!(
        "Requesting result from routine: {:?}",
        routine_client.request_result(RoutineControlRequest {
            id: "routine1".to_string(),
            request: translator
                .translate_from_uds_to_protobuf(
                    &data_items[first_data_item],
                    EntityId::Routine {
                        id: "routine1".to_string(),
                        method: RoutineMethod::RequestResult,
                        message: MethodMessage::Request
                    },
                    &[b'r', b'e', b's', b'u', b'l', b't', 0, 0, 0, 0],
                )
                .unwrap(),
            ..Default::default()
        })
    );

    // Print full diagnostics declaration of "DiagnosticsProviderService1" to verify that
    // all diagnostic declaration data is preserved at runtime.
    println!(
        "Full diagnostics declaration of DiagnosticsProviderService1: {}",
        VsidlProvider::new().unwrap().get_diagnostics_declarations()["DiagnosticsProviderService1"]
    );

    thread::sleep(Duration::from_secs(5));

    // Poll topics once:
    let mut event_subscriber = DataTunnelSubscriber::<EventTopics>::new();
    event_subscriber.subscribe().unwrap();
    let subscriber_cb = Box::new(
        |topic: &EventTopics, event: Result<Event, sdv_middleware_pubsub::error::Error>| {
            println!("Event {:?}, received from {:?}", event, topic);
        },
    );
    event_subscriber.poll(&*subscriber_cb);
}
