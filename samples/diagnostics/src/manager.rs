#![allow(missing_docs)]
// Copyright 2023 Google LLC

use diagnostics_api::connection::ConnectionMetadata;
use diagnostics_api::connection::SessionType;
use diagnostics_api::data_item_service::{ReadDataItemRequest, WriteDataItemRequest};
use diagnostics_api::diagnostics_manager_service::{
    GetConnectionMetadataRequest, GetConnectionMetadataResponse,
};
use diagnostics_api::event::Event;
use diagnostics_api::fault_listener_service::on_fault_status_change_request::StatusMask;
use diagnostics_api::fault_listener_service::OnFaultStatusChangeRequest;
use diagnostics_api::io_control_service::PerformShortTermAdjustmentRequest;
use diagnostics_api::response_code::ResponseCode;
use diagnostics_api::routine_control_service::RoutineControlRequest;
use protobuf::Message;
use protobuf::MessageField;
use protobuf::{Enum, EnumOrUnknown};
use sdv_config_provider::{VsidlConfigProvider, VsidlProvider};
use sdv_middleware_pubsub::subscriber::DataTunnelSubscriber;
use sdv_uds_serde::diagnostics_translator::DiagnosticsTranslator;
use sdv_uds_serde::entity_id::{EntityId, MethodMessage, RoutineMethod};
use sdv_uds_serde::translator::{Translator, TranslatorImpl};
use sdvgenerated::diagnostics_manager::DiagnosticsManager;
use sdvgenerated::diagnostics_manager::DiagnosticsManagerCallbacks;
use sdvgenerated::sdv_diagnostics_data_item_data_item_service_interface::DataItemService;
use sdvgenerated::sdv_diagnostics_diagnostics_manager_service_interface::DiagnosticsManagerService;
use sdvgenerated::sdv_diagnostics_fault_fault_listener_service_interface::FaultListenerService;
use sdvgenerated::sdv_diagnostics_io_control_io_control_service_interface::IoControlService;
use sdvgenerated::sdv_diagnostics_routine_control_routine_control_service_interface::RoutineControlService;
use sdvmiddleware_rpc::error::RpcError;
use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn read_data_item_request(data_item_id: &str) -> ReadDataItemRequest {
    ReadDataItemRequest { id: data_item_id.to_string(), ..Default::default() }
}

fn write_data_item_request(data_item_id: &str, value: Vec<u8>) -> WriteDataItemRequest {
    WriteDataItemRequest { id: data_item_id.to_string(), value, ..Default::default() }
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
            EventTopics::Event1 => "diagnostics_provider_service1.event.event.1",
            EventTopics::Event2 => "diagnostics_provider_service2.event.event.2",
        }
    }
}

struct DiagnosticsManagerImpl {}

impl DiagnosticsManagerImpl {
    fn new() -> Self {
        Self {}
    }
}

impl DiagnosticsManagerService for DiagnosticsManagerImpl {
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
                response_code: ResponseCode::POSITIVE_RESPONSE.into(),
                ..Default::default()
            },
            2 => GetConnectionMetadataResponse {
                response_code: ResponseCode::GENERAL_REJECT.into(),
                ..Default::default()
            },
            _ => {
                GetConnectionMetadataResponse {
                    // Set OEM-specific negative response code
                    response_code: EnumOrUnknown::<ResponseCode>::from_i32(100),
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
    let mut service = DiagnosticsManager::new(DiagnosticsManagerCallbacks {
        sdv_sample_diagnostics_data_item_via_data_tunnel_cb: Box::new(|data_item_protobuf| {
            // Create VsidlProvider
            let vsidl_provider = VsidlProvider::new_sync_provider().unwrap();

            let descriptor = vsidl_provider
                .get_message_descriptor("sdv.sample.diagnostics.DataItemViaDataTunnel")
                .unwrap();

            let translator = TranslatorImpl {};

            let uds_bytes = translator
                .translate_from_protobuf_to_uds(
                    data_item_protobuf.unwrap().write_to_bytes().unwrap().as_ref(),
                    &descriptor,
                )
                .unwrap();

            let uds_string = String::from_utf8(uds_bytes).unwrap();
            println!("Receiving Data Item via Data Tunnel, translated to UDS: {uds_string}");
        }),
    });
    service
        .sdv_diagnostics_diagnostics_manager_service_server
        .register_service_interface(Arc::new(DiagnosticsManagerImpl::new()));
    service.start().unwrap();

    // Sleep to wait for diagnostic providers to register.
    thread::sleep(Duration::from_secs(2));

    println!("Diagnostics Manager service bundle started.");

    let first_data_item_client = service
        .get_sdv_diagnostics_data_item_data_item_service_client(&data_items[first_data_item]);

    let response = first_data_item_client.read_data_item(read_data_item_request(first_data_item));
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
    //   string skipped_string = 1 [(sdv.vsidl.skip_for_diag) = true];
    //   int32 code = 2 [(sdv.vsidl.primitive_byte_size) = ONE];
    //   bool enabled = 3;
    //   string name = 4 [(sdv.vsidl.variable_type_max_size) = 5];
    // }
    //
    // fields:
    // - `skipped_string`: skipped in diag serialization
    // - `code`: byte 0
    // - `enabled`: byte 1
    // - `name`: bytes 2 to 6 inclusive, with `\0` padding
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

    let response = first_data_item_client
        .write_data_item(write_data_item_request(first_data_item, new_value.clone()));
    println!(
        "Update {:?} to a new value {:?}. Response: {:?}.",
        first_data_item, new_value, response
    );

    let response = first_data_item_client.read_data_item(read_data_item_request(first_data_item));
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

    let second_data_item_client = service
        .get_sdv_diagnostics_data_item_data_item_service_client(&data_items[second_data_item]);

    let response = second_data_item_client.read_data_item(read_data_item_request(first_data_item));
    println!(
        "Trying to fetch {:?} from the second provider, fails with response: {:?}.",
        first_data_item, response
    );

    let response = second_data_item_client.read_data_item(read_data_item_request(second_data_item));
    println!("Fetch {:?} from the second provider: {:?}.", second_data_item, response);

    let response = second_data_item_client
        .write_data_item(write_data_item_request(second_data_item, new_value.clone()));
    println!(
        "Trying to write {:?} to the second provider, fails with response: {:?}.",
        second_data_item, response
    );

    // Diagnostics routines communication:
    let routine_client = service
        .get_sdv_diagnostics_routine_control_routine_control_service_client(
            "DiagnosticsProviderService1",
        );

    println!(
        "Starting routine: {:?}",
        routine_client.start_routine(RoutineControlRequest {
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
        routine_client.stop_routine(RoutineControlRequest {
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
                    &[],
                )
                .unwrap(),
            ..Default::default()
        })
    );

    // Diagnostics io control communication:
    let io_control_client = service
        .get_sdv_diagnostics_io_control_io_control_service_client("DiagnosticsProviderService1");

    println!(
        "Requesting short term io data adjustment: {:?}",
        io_control_client.perform_short_term_adjustment(PerformShortTermAdjustmentRequest {
            id: "io_control_data1".to_string(),
            ..Default::default()
        })
    );

    // Diagnostics fault listener communication:
    let fault_listener_client = service
        .get_sdv_diagnostics_fault_fault_listener_service_client("DiagnosticsProviderService1");

    println!(
        "Sending fault status update without extended data: {:?}",
        fault_listener_client.on_fault_status_change(OnFaultStatusChangeRequest {
            id: "event.1".to_string(),
            status: (StatusMask::CONFIRMED.value() | StatusMask::WARNING_INDICATOR_ENABLED.value())
                as u32,
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
