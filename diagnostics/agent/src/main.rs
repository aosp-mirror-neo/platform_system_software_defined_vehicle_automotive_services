// Copyright 2024 Google LLC

//! Diagnostics agent implementation.
use diagnostics_v1_api::{
    data_item_service::ReadDataItemRequest, diagnostics::DiagnosticsDeclaration, event::Event,
    fault_listener_service::OnFaultStatusChangeRequest, io_control_service::IoControlRequest,
    routine_control_service::RoutineControlRequest,
};
use futures::future;
use futures::stream::StreamExt;
use log::{debug, info, warn};
use protobuf::MessageFull;
use sdv_comms::dt::{Subscriber, SubscriberOptions};
use sdv_comms::id::ServiceFqin;
use sdv_comms::rpc::{Client, ClientOptions};
use sdv_comms::sd::ServiceUnitChangeEvent;
use sdv_comms::sd::{
    ServiceDiscoveryManager, ServiceUnitDefinition, UnitNameDiscoveryArgs, UnitType,
};
use sdv_comms::ContextRef;
use sdv_config_provider::{vsidl_provider, vsidl_provider_api::VsidlConfigProviderV1};
use sdv_uds_serde::{diagnostics_translator::DiagnosticsTranslator, entity_id::EntityId};
use std::future::Future;
use std::str::FromStr;
use std::thread;
use std::time::Duration;
use std::{collections::HashSet, sync::Arc};

mod unit_types;
use crate::unit_types::*;

#[tokio::main(flavor = "multi_thread", worker_threads = 8)]
async fn main() {
    sdv_log::init_logger("sdv_diagnostics_agent").unwrap();
    sdv_tracing::init_tracing();

    sdv_comms::wait_for_agents()
        .unwrap_or_else(|err| panic!("Error waiting for Comms Stack agents: {err}"));

    let self_fqin =
        ServiceFqin::from_str("vm:com.sdv.oem.diagnostics.DiagnosticsAgent/default").unwrap();
    let context: ContextRef = ContextRef::create(self_fqin.clone());

    let vsidl_provider = Arc::new(
        vsidl_provider::new(context).unwrap_or_else(|| panic!("Error creating VSIDL provider")),
    );

    info!("Starting Diagnostics Agent");

    tokio::spawn(async move {
        info!("Starting interaction with Events");
        listen_to_events(context).await;
    });

    let cloned_vsidl_provider = vsidl_provider.clone();
    tokio::spawn(async move {
        info!("Starting interaction with DataItems");
        let vsidl_provider = cloned_vsidl_provider.clone();

        set_service_unit_registration_callback(
            context,
            DataItem::unit_type(),
            move |service_unit| {
                let service_fqin = service_unit.get_service_bundle_identity().get_fqin().clone();
                let declaration = futures_executor::block_on(
                    vsidl_provider.get_diagnostics_declaration(&service_fqin),
                );

                debug!("Diagnostics declaration of a {} : {:#?}", service_fqin, declaration,);

                tokio::spawn(read_data_items(
                    context,
                    vsidl_provider.clone(),
                    service_unit,
                    declaration.unwrap_or_default(),
                ));

                future::ready(())
            },
        )
        .await;
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
        let mut service_units = sd.list_units_by_type(&EventPublication::unit_type());

        while let Some(service_unit) = service_units.next().await {
            let sid = service_unit.get_unit_identifier().get_sid();
            if event_subscribers.contains(&sid) {
                continue;
            }

            info!(
                "Subscribing to new event: {}#{}",
                service_unit.get_service_bundle_identity().get_fqin(),
                service_unit.get_unit_identifier().get_unit_name()
            );

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

async fn read_data_items(
    context: ContextRef,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
    service_unit: ServiceUnitDefinition,
    diagnostics_declaration: DiagnosticsDeclaration,
) {
    let service_fqin = service_unit.get_service_bundle_identity().get_fqin().clone();
    let translator = DiagnosticsTranslator::new(vsidl_provider);

    for data_item in diagnostics_declaration.data_item {
        debug!("Trying to read data item: {:#?}", data_item);
        let result = perform_and_log_rpc_call::<ReadDataItem>(
            context,
            service_unit.clone(),
            ReadDataItemRequest { id: data_item.id.clone(), ..Default::default() },
        )
        .await;

        let translation_result = translator
            .translate_from_protobuf_to_uds(
                &service_fqin,
                EntityId::DataItem(data_item.id.clone()),
                &result.value,
            )
            .await;

        match translation_result {
            Ok(uds_data_item_value) => {
                debug!(
                    "Read data item: {}#{}, proto_value: {:?}, uds_value {:?}",
                    &service_fqin, data_item.id, result.value, uds_data_item_value
                );
            }
            Err(e) => {
                warn!("Error reading data item: {e}");
            }
        };
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
    let _ = perform_and_log_rpc_call::<FreezeCurrentState>(
        context,
        service_unit,
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
    let _ = perform_and_log_rpc_call::<StartRoutine>(
        context,
        service_unit,
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
    let _ = perform_and_log_rpc_call::<OnFaultStatusChange>(
        context,
        service_unit,
        OnFaultStatusChangeRequest {
            id: "lowpressure".to_string(),
            status: 1,
            ..Default::default()
        },
    )
    .await;
}

async fn set_service_unit_registration_callback<F, Fut>(context: ContextRef, args: UnitType, f: F)
where
    F: Fn(ServiceUnitDefinition) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    info!("Listening for service units registration by type : {}", args.to_string());

    let sd = ServiceDiscoveryManager::new(context);

    let unit_stream = sd.subscribe_service_unit_change_by_type(&args).await.unwrap();

    let f = Arc::new(f);

    unit_stream
        .filter_map(move |event| {
            debug!("Service unit change received: {event:#?}");

            let result = if let ServiceUnitChangeEvent::Registered(unit) = event {
                debug!(
                    "Service unit registered: {} / {}",
                    unit.get_service_bundle_identity().get_fqin(),
                    unit.get_unit_identifier().get_unit_name()
                );
                Some(unit)
            } else {
                debug!("Ignoring unit Unregister/Change events");
                None
            };

            future::ready(result)
        })
        .inspect(|unit| {
            info!(
                "Discovered unit: {} : {}",
                unit.get_service_bundle_identity().get_fqin(),
                unit.get_unit_identifier().get_unit_name()
            );

            let unit = unit.clone();
            let f = f.clone();
            tokio::spawn(async move { f(unit).await });
        })
        .collect::<Vec<_>>()
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

async fn perform_and_log_rpc_call<T: Method>(
    context: ContextRef,
    service_unit: ServiceUnitDefinition,
    request: T::Request,
) -> T::Response {
    debug!("Performing RPC call: {}:{}", T::UnitType::unit_type(), T::METHOD_NAME);

    let client = Client::create(
        context,
        service_unit.get_unit_identifier(),
        &ClientOptions::default(),
    )
    .await
    .unwrap_or_else(|cause| {
        panic!("Failed to create client for RPC call on: {service_unit:#?} with error {cause:#?}")
    });

    let response =
        client.unary_call(T::METHOD_NAME, &encode::<T::Request>(&request).unwrap()).await.unwrap();
    let response_decoded = decode::<T::Response>(&response);
    info!("Response for RPC call [{}]: {:?}", T::METHOD_NAME, response_decoded);
    response_decoded.unwrap()
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
