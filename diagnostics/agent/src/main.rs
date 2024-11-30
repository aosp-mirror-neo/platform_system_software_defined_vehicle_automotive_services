// Copyright 2024 Google LLC

//! Diagnostics agent implementation.
use diagnostics_v1_api::{
    data_item_service::ReadDataItemRequest, event::Event,
    fault_listener_service::OnFaultStatusChangeRequest, io_control_service::IoControlRequest,
    routine_control_service::RoutineControlRequest,
};
use futures::future;
use futures::stream::StreamExt;
use log::{debug, info, warn};
use protobuf::MessageFull;
use sdv_comms::dt::{SubscriberOptions, SubscriberStream};
use sdv_comms::id::ServiceFqin;
use sdv_comms::rpc::{Client, ClientOptions};
use sdv_comms::sd::ServiceUnitChangeEvent;
use sdv_comms::sd::{ServiceDiscoveryManager, ServiceUnitDefinition, UnitType};
use sdv_comms::ContextRef;
use sdv_config_provider::{vsidl_provider, vsidl_provider_api::VsidlConfigProviderV1};
use sdv_uds_serde::{diagnostics_translator::DiagnosticsTranslator, entity_id::EntityId};
use std::future::Future;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

mod unit_types;
use crate::unit_types::*;

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
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

    tokio::spawn(listen_to_events(context));

    tokio::spawn(data_item_interaction(context, vsidl_provider.clone()));
    tokio::spawn(io_control_interaction(context, vsidl_provider.clone()));
    tokio::spawn(routine_control_interaction(context, vsidl_provider.clone()));
    tokio::spawn(fault_listener_interaction(context, vsidl_provider.clone()));

    binder::ProcessState::start_thread_pool();

    binder::ProcessState::join_thread_pool();
}

async fn listen_to_events(context: ContextRef) {
    info!("Starting interaction with Events");
    set_service_unit_registration_callback(
        context,
        EventPublication::unit_type(),
        move |service_unit| {
            tokio::spawn(async move {
                for _ in 0..20 {
                    let dt_stream = SubscriberStream::create(
                        context,
                        &service_unit.get_unit_identifier(),
                        Duration::ZERO,
                        SubscriberOptions { fetch_last_message: true },
                    )
                    .await;

                    if let Ok(dt_stream) = dt_stream {
                        dt_stream
                            .for_each(|packet| {
                                match packet {
                                    Ok((msg_count, buffer)) => handle_events(msg_count, buffer),
                                    Err(e) => {
                                        panic!("[SubscriberStream] message read failed {}", e)
                                    }
                                };

                                futures::future::ready(())
                            })
                            .await;

                        break;
                    } else {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            });

            future::ready(())
        },
    )
    .await;
}

async fn read_data_items(
    context: ContextRef,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
    service_unit: ServiceUnitDefinition,
) {
    let service_fqin = service_unit.get_service_bundle_identity().get_fqin().clone();
    let translator = DiagnosticsTranslator::new(vsidl_provider.clone());

    let diagnostics_declaration =
        vsidl_provider.get_diagnostics_declaration(&service_fqin).await.unwrap_or_default();

    debug!(
        "DataItems diagnostics declaration of a {} : {:#?}",
        service_fqin, diagnostics_declaration.data_item
    );

    for data_item in diagnostics_declaration.data_item {
        debug!("Trying to read data item: {:#?}", data_item);
        let result = perform_and_log_rpc_call::<ReadDataItem>(
            context,
            &service_unit,
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

async fn start_routines(
    context: ContextRef,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
    service_unit: ServiceUnitDefinition,
) {
    let service_fqin = service_unit.get_service_bundle_identity().get_fqin().clone();
    let diagnostics_declaration =
        vsidl_provider.get_diagnostics_declaration(&service_fqin).await.unwrap_or_default();

    debug!(
        "RoutinControls diagnostics declaration of a {} : {:#?}",
        service_fqin, diagnostics_declaration.routine
    );

    for routine in diagnostics_declaration.routine {
        debug!("Trying to start routine: {:#?}", routine);
        perform_and_log_rpc_call::<StartRoutine>(
            context,
            &service_unit,
            RoutineControlRequest { id: "pressurization".to_string(), ..Default::default() },
        )
        .await;
    }
}

async fn freeze_current_state(
    context: ContextRef,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
    service_unit: ServiceUnitDefinition,
) {
    let service_fqin = service_unit.get_service_bundle_identity().get_fqin().clone();
    let diagnostics_declaration =
        vsidl_provider.get_diagnostics_declaration(&service_fqin).await.unwrap_or_default();

    debug!(
        "IoControls diagnostics declaration of a {} : {:#?}",
        service_fqin, diagnostics_declaration.io_control
    );

    for io_control in diagnostics_declaration.io_control {
        debug!("Trying to freeze current state for: {:#?}", io_control);
        perform_and_log_rpc_call::<FreezeCurrentState>(
            context,
            &service_unit,
            IoControlRequest { id: io_control.id.to_string(), ..Default::default() },
        )
        .await;
    }
}

async fn update_faults_status(
    context: ContextRef,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
    service_unit: ServiceUnitDefinition,
) {
    let service_fqin = service_unit.get_service_bundle_identity().get_fqin().clone();
    let diagnostics_declaration =
        vsidl_provider.get_diagnostics_declaration(&service_fqin).await.unwrap_or_default();

    debug!(
        "FaultListeners diagnostics declaration of a {} : {:#?}",
        service_fqin, diagnostics_declaration.routine
    );

    for routine in diagnostics_declaration.routine {
        debug!("Trying to start routine: {:#?}", routine);
        perform_and_log_rpc_call::<OnFaultStatusChange>(
            context,
            &service_unit,
            OnFaultStatusChangeRequest {
                id: routine.id.to_string(),
                status: 1,
                ..Default::default()
            },
        )
        .await;
    }
}

fn handle_events(available_to_read: u32, bytes: Vec<u8>) {
    info!("Available to read: {available_to_read}");

    let size_for_one_msg = bytes.len() / available_to_read as usize;

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

async fn data_item_interaction(
    context: ContextRef,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
) {
    info!("Starting interaction with DataItems");

    set_service_unit_registration_callback(context, DataItem::unit_type(), move |service_unit| {
        tokio::spawn(read_data_items(context, vsidl_provider.clone(), service_unit));

        future::ready(())
    })
    .await;
}

async fn io_control_interaction(
    context: ContextRef,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
) {
    info!("Starting interaction with IoControls");

    set_service_unit_registration_callback(context, IoControl::unit_type(), move |service_unit| {
        tokio::spawn(freeze_current_state(context, vsidl_provider.clone(), service_unit));

        future::ready(())
    })
    .await;
}

async fn routine_control_interaction(
    context: ContextRef,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
) {
    info!("Starting interaction with RoutineControls");

    set_service_unit_registration_callback(
        context,
        RoutineControl::unit_type(),
        move |service_unit| {
            tokio::spawn(start_routines(context, vsidl_provider.clone(), service_unit));

            future::ready(())
        },
    )
    .await;
}

async fn fault_listener_interaction(
    context: ContextRef,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
) {
    info!("Starting interaction with FaultListeners");

    set_service_unit_registration_callback(
        context,
        FaultListener::unit_type(),
        move |service_unit| {
            tokio::spawn(update_faults_status(context, vsidl_provider.clone(), service_unit));

            future::ready(())
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
        .for_each(|unit: ServiceUnitDefinition| {
            info!(
                "Discovered unit: {} : {}",
                unit.get_service_bundle_identity().get_fqin(),
                unit.get_unit_identifier().get_unit_name()
            );

            tokio::spawn(f(unit.clone()));

            future::ready(())
        })
        .await
}

async fn perform_and_log_rpc_call<T: Method>(
    context: ContextRef,
    service_unit: &ServiceUnitDefinition,
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
