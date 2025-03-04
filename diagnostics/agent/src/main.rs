// Copyright 2024 Google LLC

//! Diagnostics agent implementation.
use diagnostics_v1_api::event::{EnableCondition, OperationCycle};
use diagnostics_v1_api::{
    data_item_service::ReadDataItemRequest, event::Event,
    fault_listener_service::OnFaultStatusChangeRequest, io_control_service::IoControlRequest,
    routine_control_service::RoutineControlRequest,
};
use futures::future;
use futures::stream::StreamExt;
use log::{debug, error, info, warn};
use protobuf::MessageFull;
use sdv_comms::dt::{SubscriberOptions, SubscriberStream};
use sdv_comms::id::ServiceFqin;
use sdv_comms::rpc::{Client, ClientOptions};
use sdv_comms::sd::ServiceUnitChangeEvent;
use sdv_comms::sd::{ServiceDiscoveryManager, ServiceUnitDefinition, UnitType};
use sdv_comms::ContextRef;
use sdv_config_provider::{vsidl_provider, vsidl_provider_api::VsidlConfigProviderV1};
use sdv_status_rs::{SdvResult, SdvStatus, SdvStatusCode};
use sdv_uds_serde::{diagnostics_translator::DiagnosticsTranslator, entity_id::*};
use std::future::Future;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

mod unit_types;
use crate::unit_types::*;

use protobuf::MessageDyn;
use tracing::instrument;

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
#[instrument]
async fn main() {
    sdv_log::init_logger("sdv_diagnostics_agent").unwrap();
    sdv_tracing::init_tracing();

    // Emulate opening external connection
    let _ = tokio::net::TcpListener::bind("127.0.0.1:8080").await.unwrap();

    log::debug!("sdv_diagnostics_agent accepts external connections");
    tracing::event!(tracing::Level::DEBUG, "sdv_diagnostics_agent accepts external connections");

    sdv_comms::wait_for_agents()
        .unwrap_or_else(|err| panic!("Error waiting for Comms Stack agents: {err}"));

    let self_fqin =
        ServiceFqin::from_str("vm:com.sdv.oem.diagnostics.DiagnosticsAgent/default").unwrap();
    let context: ContextRef = ContextRef::create(self_fqin.clone());

    let vsidl_provider = Arc::new(
        vsidl_provider::new(context).unwrap_or_else(|| panic!("Error creating VSIDL provider")),
    );

    log::debug!("sdv_diagnostics_agent VSIDL provider initialized");
    tracing::event!(tracing::Level::DEBUG, "sdv_diagnostics_agent VSIDL provider initialized");

    info!("Starting Diagnostics Agent");

    tokio::spawn(listen_to_enable_conditions(context, vsidl_provider.clone()));
    tokio::spawn(listen_to_operation_cycles(context, vsidl_provider.clone()));
    tokio::spawn(listen_to_events(context, vsidl_provider.clone()));

    tokio::spawn(data_item_interaction(context, vsidl_provider.clone()));
    tokio::spawn(io_control_interaction(context, vsidl_provider.clone()));
    tokio::spawn(routine_control_interaction(context, vsidl_provider.clone()));
    tokio::spawn(fault_listener_interaction(context, vsidl_provider.clone()));

    log::debug!("sdv_diagnostics_agent has started successfully");
    tracing::event!(tracing::Level::DEBUG, "sdv_diagnostics_agent has started successfully");

    binder::ProcessState::start_thread_pool();

    let () = future::pending().await;
}

async fn listen_to_all_publications_of_type<T: PublicationUnitType>(
    context: ContextRef,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
    handler: impl Fn(ServiceUnitDefinition, Arc<dyn VsidlConfigProviderV1>, T::Message)
        + Send
        + Sync
        + Clone
        + 'static,
) {
    set_service_unit_registration_callback(
        context,
        T::unit_type(),
        move|service_unit| {

            let service_unit = service_unit.clone();
            let handler = handler.clone();
            let vsidl_provider = vsidl_provider.clone();

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
                                    Ok((msg_count, buffer)) => {
                                        info!("Available to read: {msg_count}");

                                        if msg_count != 0 {
                                            let size_for_one_msg = buffer.len() / msg_count as usize;

                                            let messages_bytes = buffer
                                                .chunks(size_for_one_msg)
                                                .map(|chunk| chunk.to_owned().into_boxed_slice())
                                                .collect::<Vec<_>>();

                                            for message_bytes in &messages_bytes {
                                                let event = decode_with_len::<T::Message>(message_bytes);

                                                if let Ok(event) = event {
                                                    handler(service_unit.clone(), vsidl_provider.clone(), event)
                                                } else {
                                                    warn!("Failed to parse publication of type {} for {service_unit:?}", T::TYPE_NAME)
                                                }
                                            }
                                        }
                                    },
                                    Err(e) => {
                                        warn!("Publisher unregistered: {:#?}", e)
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

async fn listen_to_events(context: ContextRef, vsidl_provider: Arc<dyn VsidlConfigProviderV1>) {
    info!("Starting interaction with Events");
    listen_to_all_publications_of_type::<EventPublication>(context, vsidl_provider, handle_event)
        .await
}

async fn listen_to_enable_conditions(
    context: ContextRef,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
) {
    info!("Starting interaction with EnableConditions");
    listen_to_all_publications_of_type::<EnableConditionPublication>(
        context,
        vsidl_provider,
        handle_enable_condition,
    )
    .await;
}

async fn listen_to_operation_cycles(
    context: ContextRef,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
) {
    info!("Starting interaction with OperationCycles");
    listen_to_all_publications_of_type::<OperationCyclePublication>(
        context,
        vsidl_provider,
        handle_operation_cycle,
    )
    .await;
}

#[instrument(skip_all)]
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

        let Ok(result) = result else {
            error!("Failed to perform rpc call");
            continue;
        };

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

#[instrument(skip_all)]
async fn start_routines(
    context: ContextRef,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
    service_unit: ServiceUnitDefinition,
) {
    let service_fqin = service_unit.get_service_bundle_identity().get_fqin().clone();
    let diagnostics_declaration =
        vsidl_provider.get_diagnostics_declaration(&service_fqin).await.unwrap_or_default();
    let translator = DiagnosticsTranslator::new(vsidl_provider.clone());

    debug!(
        "RoutinControls diagnostics declaration of a {} : {:#?}",
        service_fqin, diagnostics_declaration.routine
    );

    for routine in diagnostics_declaration.routine {
        debug!("Trying to start routine: {:#?}", routine);
        let response = perform_and_log_rpc_call::<StartRoutine>(
            context,
            &service_unit,
            RoutineControlRequest { id: routine.id.to_string(), ..Default::default() },
        )
        .await;

        let Ok(response) = response else {
            error!("Failed to perform rpc call");
            continue;
        };

        let translation_result = translator
            .translate_from_protobuf_to_uds(
                &service_fqin,
                EntityId::Routine {
                    id: routine.id.clone(),
                    method: RoutineMethod::Start,
                    message: MethodMessage::Response,
                },
                &response.response,
            )
            .await;

        match translation_result {
            Ok(uds_response) => {
                debug!(
                    "Start routine: {}#{}, proto_value: {:?}, uds_value {:?}",
                    &service_fqin, routine.id, response.response, uds_response
                );
            }
            Err(e) => {
                warn!("Error starting routine: {e}");
            }
        };
    }
}

#[instrument(skip_all)]
async fn freeze_current_state(
    context: ContextRef,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
    service_unit: ServiceUnitDefinition,
) {
    let service_fqin = service_unit.get_service_bundle_identity().get_fqin().clone();
    let diagnostics_declaration =
        vsidl_provider.get_diagnostics_declaration(&service_fqin).await.unwrap_or_default();
    let translator = DiagnosticsTranslator::new(vsidl_provider.clone());

    debug!(
        "IoControls diagnostics declaration of a {} : {:#?}",
        service_fqin, diagnostics_declaration.io_control
    );

    for io_control in diagnostics_declaration.io_control {
        debug!("Trying to freeze current state for: {:#?}", io_control);
        let response = perform_and_log_rpc_call::<FreezeCurrentState>(
            context,
            &service_unit,
            IoControlRequest { id: io_control.id.to_string(), ..Default::default() },
        )
        .await;

        let Ok(response) = response else {
            error!("Failed to perform rpc call");
            continue;
        };

        let translation_result = translator
            .translate_from_protobuf_to_uds(
                &service_fqin,
                EntityId::InputOutputData(io_control.id.clone()),
                &response.value,
            )
            .await;

        match translation_result {
            Ok(uds_data_item_value) => {
                debug!(
                    "Freeze current state for: {}#{}, proto_response: {:?}, uds_response: {:?}",
                    &service_fqin, io_control.id, response.value, uds_data_item_value
                );
            }
            Err(e) => {
                warn!("Error freezing current state: {e}");
            }
        };
    }
}

#[instrument(skip_all)]
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
        service_fqin, diagnostics_declaration.event
    );

    for event in diagnostics_declaration.event {
        debug!("Trying to update fault status: {:#?}", event);
        let _ = perform_and_log_rpc_call::<OnFaultStatusChange>(
            context,
            &service_unit,
            OnFaultStatusChangeRequest {
                id: event.id.to_string(),
                status: 1,
                ..Default::default()
            },
        )
        .await;
    }
}

#[instrument(skip_all)]
fn handle_event(
    service_unit: ServiceUnitDefinition,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
    event: Event,
) {
    info!("Event received: {:#?}", event);

    if let Some(ref extended_data) = event.extended_data {
        info!("Event extended data received: {:#?}", extended_data);

        let unit_name = service_unit.get_unit_identifier().get_unit_name().to_string();

        let translator = DiagnosticsTranslator::new(vsidl_provider.clone());

        let service_fqin = service_unit.get_service_bundle_identity().get_fqin().clone();
        let diagnostics_declaration =
            futures_executor::block_on(vsidl_provider.get_diagnostics_declaration(&service_fqin))
                .unwrap_or_default();

        let event_declaration =
            diagnostics_declaration.event.iter().find(|event| event.service_unit_name == unit_name);

        if let Some(event_declaration) = event_declaration {
            info!("Found declaration for event: {event_declaration}");
            if let Some(data) = event.extended_data {
                let uds_extended_data =
                    futures_executor::block_on(translator.translate_from_protobuf_to_uds(
                        &service_fqin,
                        EntityId::EventExtendedData(event_declaration.id.clone()),
                        &data,
                    ));
                info!("Event extended data in UDS format: {uds_extended_data:#?}")
            }
        } else {
            warn!("Didn't find declaration for event: {service_unit:#?}");
        }
    }
}

#[instrument(skip_all)]
fn handle_enable_condition(
    _service_unit: ServiceUnitDefinition,
    _vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
    enable_condition: EnableCondition,
) {
    info!("Enable condition change received: {:#?}", enable_condition);
}

fn handle_operation_cycle(
    _service_unit: ServiceUnitDefinition,
    _vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
    operation_cycle: OperationCycle,
) {
    info!("Operaction cycle command received: {:#?}", operation_cycle);
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

#[instrument(skip_all)]
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

#[instrument(skip_all)]
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

#[instrument(skip_all)]
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

#[instrument(skip_all)]
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

#[instrument(skip_all, fields(request = request.descriptor_dyn().name()))]
async fn perform_and_log_rpc_call<T: Method>(
    context: ContextRef,
    service_unit: &ServiceUnitDefinition,
    request: T::Request,
) -> SdvResult<T::Response> {
    debug!("Performing RPC call: {}:{}", T::UnitType::unit_type(), T::METHOD_NAME);

    let client =
        Client::create(context, service_unit.get_unit_identifier(), &ClientOptions::default())
            .await?;

    let encoded_request = encode::<T::Request>(&request).map_err(|e| {
        SdvStatus::with_message(
            SdvStatusCode::Internal,
            format!("Failed to encode a request: {e:#?}"),
        )
    })?;

    let response = client.unary_call(T::METHOD_NAME, &encoded_request).await?;
    let response_decoded = decode::<T::Response>(&response);
    info!("Response for RPC call [{}]: {:?}", T::METHOD_NAME, response_decoded);
    response_decoded.map_err(|e| {
        SdvStatus::with_message(
            SdvStatusCode::Internal,
            format!("Failed to decode a response: {e:#?}"),
        )
    })
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
