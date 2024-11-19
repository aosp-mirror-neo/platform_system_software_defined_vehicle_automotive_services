// Copyright 2024 Google LLC

//! Diagnostics agent implementation.

use diagnostics_v1_api::{
    event::Event,
    fault_listener_service::{OnFaultStatusChangeRequest, OnFaultStatusChangeResponse},
    io_control_service::{IoControlRequest, IoControlResponse},
    data_item_service::{ReadDataItemRequest, ReadDataItemResponse},
    routine_control_service::{RoutineControlRequest, RoutineControlResponse},
};
use futures::stream::StreamExt;
use log::info;
use protobuf::MessageFull;
use sdv_comms::dt::{Subscriber, SubscriberOptions};
use sdv_comms::id::ServiceFqin;
use sdv_comms::rpc::{Client, ClientOptions};
use sdv_comms::sd::{
    ServiceDiscoveryManager, ServiceUnitDefinition, UnitNameDiscoveryArgs, UnitType,
};
use sdv_comms::ContextRef;
use std::collections::HashSet;
use std::str::FromStr;
use std::thread;
use std::time::Duration;
use google_sdv_rpc_aidl::aidl::google::sdv::rpc::IRpcAgent::{BpRpcAgent, IRpcAgent};
use google_sdv_service_discovery_discovery_aidl::aidl::google::sdv::service_discovery::discovery::IServiceDiscoveryAgent::{BpServiceDiscoveryAgent, IServiceDiscoveryAgent};
use sdv_comms::sd::ServiceUnitChangeEvent;
use futures::future;
use sdv_config_provider::{
    vsidl_provider::VsidlProviderV1,
    vsidl_provider_api::VsidlConfigProviderV1,
};

#[tokio::main]
async fn main() {
    sdv_log::init_logger("sdv_diagnostics_agent").unwrap();
    sdv_tracing::init_tracing();

    wait_for_comms_stack();

    let self_fqin =
        ServiceFqin::from_str("vm:com.sdv.google.diagnostics.DiagnosticsAgent/default").unwrap();
    let context = ContextRef::create(self_fqin.clone());

    use_vsidl_provider();

    info!("Starting Diagnostics Agent");

    tokio::spawn(async move {
        info!("Starting interaction with Events");
        listen_to_events(context).await;
    });

    tokio::spawn(async move {
        info!("Starting interaction with DataItems");
        data_item_interaction(context).await;
    });

    tokio::spawn(async move {
        info!("Starting interaction with IoControls");
        io_control_interaction(context).await;
    });

    tokio::spawn(async move {
        info!("Starting interaction with RoutineControls");
        routine_control_interaction(context).await;
    });

    tokio::spawn(async move {
        info!("Starting interaction with FaultListeners");
        fault_listener_interaction(context).await;
    });

    binder::ProcessState::start_thread_pool();

    binder::ProcessState::join_thread_pool();
}

async fn listen_to_events(context: ContextRef) {
    let mut event_subscribers = HashSet::new();
    let sd = ServiceDiscoveryManager::new(context);

    loop {
        let mut service_units = sd.list_units_by_type(
            &UnitType::builder()
                .set_sdv_package_name("com.sdv.google.diagnostics")
                .set_service_bundle_name("DiagnosticsManager")
                .set_unit_type_name("event.Event")
                .build()
                .unwrap(),
        );

        while let Some(service_unit) = service_units.next().await {
            let sid = service_unit.get_unit_identifier().get_sid();
            if event_subscribers.contains(&sid) {
                continue;
            }

            info!("Subscribing to new event: {service_unit:#?}");

            let uid = service_unit.get_unit_identifier();
            let subscriber =
                Subscriber::create(context, &uid, SubscriberOptions { fetch_last_message: true })
                    .await
                    .unwrap();

            tokio::spawn(async move {
                subscribe_to_event(subscriber).await;
            });

            event_subscribers.insert(sid);
        }

        thread::sleep(Duration::from_millis(200));
    }
}

async fn subscribe_to_event(subscriber: Subscriber) {
    let size_for_one_msg = subscriber.get_publisher_unit_metadata().get_message_size() as usize;

    info!("Subscribe to event with msg size: {size_for_one_msg}");

    loop {
        let available_to_read = subscriber.available_to_read().unwrap_or(0) as usize;

        if available_to_read > 0 {
            info!("Available to read: {available_to_read}");
            let mut bytes = vec![0u8; size_for_one_msg * available_to_read];
            let _ = subscriber.read_messages(&mut bytes);

            let messages_bytes = bytes
                .chunks(size_for_one_msg)
                .map(|chunk| chunk.to_owned().into_boxed_slice())
                .collect::<Vec<_>>();

            for message_bytes in &messages_bytes {
                let event = decode_with_len::<Event>(message_bytes);
                info!("Event received: {:#?}", event);

                if let Some(ref extended_data) = event.unwrap().extended_data {
                    // TODO(372607401): use VsidlProvider to parse the extended data message.
                    info!("Event extended data received: {:#?}", extended_data);
                }
            }
        }

        thread::sleep(Duration::from_secs(1));
    }
}

async fn data_item_interaction(context: ContextRef) {
    let service_unit = wait_for_service_unit(
        context,
        UnitNameDiscoveryArgs::new_builder()
            .set_sdv_package_name("com.sdv.oem.sample.diagnostics")
            .set_service_bundle_name("DiagnosticsSampleWithDataItem")
            .set_service_unit_name("com-sdv-google-diagnostics-data-item-data-item-service")
            .build()
            .unwrap(),
    )
    .await;

    // TODO(372607401): use VsidlProvider to parse the response message.
    let _ = perform_and_log_rpc_call::<ReadDataItemRequest, ReadDataItemResponse>(
        context,
        service_unit,
        "ReadDataItem",
        ReadDataItemRequest { id: "pressure".to_string(), ..Default::default() },
    )
    .await;
}

async fn io_control_interaction(context: ContextRef) {
    let service_unit = wait_for_service_unit(
        context,
        UnitNameDiscoveryArgs::new_builder()
            .set_sdv_package_name("com.sdv.oem.sample.diagnostics")
            .set_service_bundle_name("DiagnosticsProvider1")
            .set_service_unit_name("com-sdv-google-diagnostics-io-control-io-control-service")
            .build()
            .unwrap(),
    )
    .await;

    // TODO(372607401): use VsidlProvider to parse the response message.
    let _ = perform_and_log_rpc_call::<IoControlRequest, IoControlResponse>(
        context,
        service_unit,
        "FreezeCurrentState",
        IoControlRequest { id: "pressure".to_string(), ..Default::default() },
    )
    .await;
}

async fn routine_control_interaction(context: ContextRef) {
    let service_unit = wait_for_service_unit(
        context,
        UnitNameDiscoveryArgs::new_builder()
            .set_sdv_package_name("com.sdv.oem.sample.diagnostics")
            .set_service_bundle_name("DiagnosticsProvider1")
            .set_service_unit_name(
                "com-sdv-google-diagnostics-routine-control-routine-control-service",
            )
            .build()
            .unwrap(),
    )
    .await;

    // TODO(372607401): use VsidlProvider to parse the response message.
    let _ = perform_and_log_rpc_call::<RoutineControlRequest, RoutineControlResponse>(
        context,
        service_unit,
        "StartRoutine",
        RoutineControlRequest { id: "pressurization".to_string(), ..Default::default() },
    )
    .await;
}

async fn fault_listener_interaction(context: ContextRef) {
    let service_unit = wait_for_service_unit(
        context,
        UnitNameDiscoveryArgs::new_builder()
            .set_sdv_package_name("com.sdv.oem.sample.diagnostics")
            .set_service_bundle_name("DiagnosticsProvider1")
            .set_service_unit_name("com-sdv-google-diagnostics-fault-fault-listener-service")
            .build()
            .unwrap(),
    )
    .await;

    // TODO(372607401): use VsidlProvider to parse the response message.
    let _ = perform_and_log_rpc_call::<OnFaultStatusChangeRequest, OnFaultStatusChangeResponse>(
        context,
        service_unit,
        "OnFaultStatusChange",
        OnFaultStatusChangeRequest {
            id: "lowpressure".to_string(),
            status: 1,
            ..Default::default()
        },
    )
    .await;
}

async fn wait_for_service_unit(
    context: ContextRef,
    args: UnitNameDiscoveryArgs,
) -> ServiceUnitDefinition {
    info!("Waiting for service unit: {:?}", args.get_unit_name());
    let sd = ServiceDiscoveryManager::new(context);

    let unit_event_stream = sd.subscribe_service_unit_change_by_name(&args).await.unwrap();

    unit_event_stream
        .filter_map(move |event| {
            info!("Event received: {event:#?}");

            let result = if let ServiceUnitChangeEvent::Registered(unit) = event {
                Some(unit)
            } else {
                None
            };

            thread::sleep(Duration::from_secs(3));

            future::ready(result)
        })
        .next()
        .await
        .inspect(|unit| {
            info!(
                "Discovered unit: {} : {}",
                unit.get_service_bundle_identity().get_fqin(),
                unit.get_unit_identifier().get_unit_name()
            )
        })
        .unwrap()
}

async fn perform_and_log_rpc_call<Req: MessageFull, Resp: MessageFull>(
    context: ContextRef,
    service_unit: ServiceUnitDefinition,
    method: &'static str,
    request: Req,
) -> Resp {
    let client = Client::create(
        context,
        service_unit.get_unit_identifier(),
        &ClientOptions::default(),
    )
    .await
    .unwrap_or_else(|cause| {
        panic!("Failed to create client for RPC call on: {service_unit:#?} with error {cause:#?}")
    });
    let response = client.unary_call(method, &encode::<Req>(&request).unwrap()).await.unwrap();
    let response_decoded = decode::<Resp>(&response);
    info!("Response for RPC call [{}]: {:?}", method, response_decoded);
    response_decoded.unwrap()
}

fn wait_for_comms_stack() {
    let descriptor = <BpServiceDiscoveryAgent as IServiceDiscoveryAgent>::get_descriptor()
        .to_owned()
        + "/default";
    with_retry(|| binder::wait_for_interface::<dyn IServiceDiscoveryAgent>(&descriptor));

    let descriptor = <BpRpcAgent as IRpcAgent>::get_descriptor().to_owned() + "/default";
    with_retry(|| binder::wait_for_interface::<dyn IRpcAgent>(&descriptor));
    thread::sleep(Duration::from_secs(10));
}

fn with_retry<T, E, F>(function: F) -> T
where
    F: Fn() -> Result<T, E>,
{
    info!("Retrying wait for comms stack");
    function().unwrap_or_else(|_| with_retry(function))
}

fn encode<M: MessageFull>(message: &M) -> Result<Vec<u8>, protobuf::Error> {
    message.write_to_bytes()
}

fn decode<T: MessageFull>(message_bytes: &[u8]) -> Result<T, protobuf::Error> {
    T::parse_from_bytes(message_bytes)
}

fn decode_with_len<T: MessageFull>(message_bytes: &[u8]) -> Result<T, protobuf::Error> {
    let mut input_stream = protobuf::CodedInputStream::from_bytes(message_bytes);
    let size = input_stream.read_raw_varint32().unwrap().into();
    // And parse only the part of the byte array that contains the message
    let _ = input_stream.push_limit(size);

    T::parse_from(&mut input_stream)
}

fn use_vsidl_provider() {
    let vsidl_provider = VsidlProviderV1::new_sync_provider().unwrap();

    // Declaration from APEX.
    log::debug!(
        "Diagnostics declaration: {:#?}",
        vsidl_provider.get_diagnostics_declaration(
            &ServiceFqin::from_str(
                "vm:com.sdv.oem.sample.diagnostics.DiagnosticsSampleWithDataItem/default"
            )
            .unwrap()
        )
    );
}
