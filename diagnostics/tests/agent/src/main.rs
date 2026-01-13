/*
 * Copyright (C) 2026 The Android Open Source Project
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

//! Diagnostics agent implementation.
use diagnostics_v1_api::event::{EnableCondition, OperationCycle};
use diagnostics_v1_api::routine_control_service::RoutineControlResponse;
use diagnostics_v1_api::{
    data_item_service::ReadDataItemRequest, event::Event,
    fault_listener_service::OnFaultStatusChangeRequest, io_control_service::IoControlRequest,
    routine_control_service::RoutineControlRequest,
};
use futures::future;
use futures::stream::StreamExt;
use log::{debug, info, warn};
use protobuf::MessageDyn;
use protobuf::MessageFull;
use sdv_comms::dt::{SubscriberOptions, SubscriberStream};
use sdv_comms::id::ServiceFqin;
use sdv_comms::rpc::{Client, ClientOptions};
use sdv_comms::sd::ServiceUnitChangeEvent;
use sdv_comms::sd::{ServiceDiscoveryManager, ServiceUnitDefinition, UnitType};
use sdv_comms::ContextRef;
use sdv_config_provider::{vsidl_provider, vsidl_provider_api::VsidlConfigProviderV1};
use sdv_dump::DumpBinderConnection;
use sdv_status_rs::{SdvResult, SdvStatus, SdvStatusCode};
use sdv_uds_serde::{diagnostics_translator::DiagnosticsTranslator, entity_id::*};
use std::fmt;
use std::fmt::Display;
use std::future::Future;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tracing::Instrument;

mod unit_types;
use crate::unit_types::*;

use tracing::instrument;

static BINDER_TAG: &str = "diagnostics";
static DUMP_REPORT_TITLE: &str = "SDV Agent dump - Diagnostics";

#[derive(Default)]
struct Logger {
    lines: Mutex<Vec<String>>,
}

impl Logger {
    fn add_line(&self, line: String) {
        self.lines.lock().unwrap().push(line);
    }

    fn new() -> Self {
        Self { ..Default::default() }
    }
}

impl Display for Logger {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for line in self.lines.lock().unwrap().iter() {
            writeln!(f, "{line}")?;
        }
        Ok(())
    }
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
#[instrument]
async fn main() -> anyhow::Result<()> {
    sdv_log::init_logger("sdv_diagnostics_agent")?;
    sdv_tracing::init_tracing();

    let dm = DumpBinderConnection::initialize(DUMP_REPORT_TITLE, BINDER_TAG);
    let dumpsys_logger = Arc::new(Logger::new());
    dm.add_reporter("Log", &dumpsys_logger);

    // Emulate opening external connection
    create_tcp_connection().await;
    info!("sdv_diagnostics_agent accepts external connections");

    dumpsys_logger.add_line("sdv_diagnostics_agent accepts external connections".to_owned());
    tracing::event!(tracing::Level::DEBUG, "sdv_diagnostics_agent accepts external connections");
    sdv_comms::wait_for_agents()
        .unwrap_or_else(|err| panic!("Error waiting for Comms Stack agents: {err}"));

    let self_fqin = ServiceFqin::from_str("vm:com.sdv.oem.diagnostics.DiagnosticsAgent/default")?;
    let context: ContextRef = ContextRef::create(self_fqin.clone());

    dm.set_self_fqin(&self_fqin);

    let vsidl_provider = Arc::new(vsidl_provider::new_for_diagnostics_agent(context));

    info!("sdv_diagnostics_agent VSIDL provider initialized");
    dumpsys_logger.add_line("sdv_diagnostics_agent VSIDL provider initialized".to_owned());
    tracing::event!(tracing::Level::DEBUG, "sdv_diagnostics_agent VSIDL provider initialized");

    dumpsys_logger.add_line("Starting Diagnostics Agent".to_owned());

    let dumpsys_logger_listen_to_enable_conditions: Arc<Logger> = Arc::new(Logger::new());
    let dumpsys_logger_listen_to_operation_cycles: Arc<Logger> = Arc::new(Logger::new());
    let dumpsys_logger_listen_to_events: Arc<Logger> = Arc::new(Logger::new());
    let dumpsys_logger_data_item_interaction: Arc<Logger> = Arc::new(Logger::new());
    let dumpsys_logger_io_control_interaction: Arc<Logger> = Arc::new(Logger::new());
    let dumpsys_logger_routine_control_interaction: Arc<Logger> = Arc::new(Logger::new());
    let dumpsys_logger_fault_listener_interaction: Arc<Logger> = Arc::new(Logger::new());
    dm.add_reporter("Log Listen to Enable Conditions", &dumpsys_logger_listen_to_enable_conditions);
    dm.add_reporter("Log Listen to Operation Cycles", &dumpsys_logger_listen_to_operation_cycles);
    dm.add_reporter("Log Listen to Events", &dumpsys_logger_listen_to_events);
    dm.add_reporter("Log Listen to Data Item Interaction", &dumpsys_logger_data_item_interaction);
    dm.add_reporter("Log Listen to IO Control Interaction", &dumpsys_logger_io_control_interaction);
    dm.add_reporter(
        "Log Listen to Routing Control Interaction",
        &dumpsys_logger_routine_control_interaction,
    );
    dm.add_reporter(
        "Log Listen to Faukt Listener Interaction",
        &dumpsys_logger_fault_listener_interaction,
    );

    tokio::spawn(listen_to_enable_conditions(
        context,
        vsidl_provider.clone(),
        dumpsys_logger_listen_to_enable_conditions.clone(),
    ));
    tokio::spawn(listen_to_operation_cycles(
        context,
        vsidl_provider.clone(),
        dumpsys_logger_listen_to_operation_cycles.clone(),
    ));
    tokio::spawn(listen_to_events(
        context,
        vsidl_provider.clone(),
        dumpsys_logger_listen_to_events.clone(),
    ));
    tokio::spawn(data_item_interaction(
        context,
        vsidl_provider.clone(),
        dumpsys_logger_data_item_interaction.clone(),
    ));
    tokio::spawn(io_control_interaction(
        context,
        vsidl_provider.clone(),
        dumpsys_logger_io_control_interaction.clone(),
    ));
    tokio::spawn(routine_control_interaction(
        context,
        vsidl_provider.clone(),
        dumpsys_logger_routine_control_interaction.clone(),
    ));
    tokio::spawn(fault_listener_interaction(
        context,
        vsidl_provider.clone(),
        dumpsys_logger_fault_listener_interaction.clone(),
    ));

    info!("sdv_diagnostics_agent has started successfully");
    dumpsys_logger.add_line("sdv_diagnostics_agent has started successfully".to_owned());

    tracing::event!(tracing::Level::DEBUG, "sdv_diagnostics_agent has started successfully");
    dm.set_agent_state("sdv_diagnostics_agent has started successfully");

    binder::ProcessState::start_thread_pool();

    let () = future::pending().await;
    Ok(())
}

async fn listen_to_all_publications_of_type<T: PublicationUnitType>(
    context: ContextRef,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
    handler: impl Fn(ServiceUnitDefinition, Arc<dyn VsidlConfigProviderV1>, T::Message, Arc<Logger>)
        + Send
        + Sync
        + Clone
        + 'static,
    dumpsys_logger: Arc<Logger>,
) {
    let dumpsys_logger_inner = dumpsys_logger.clone();

    set_service_unit_registration_callback(
        context,
        T::unit_type(),
        move|service_unit| {

            let service_unit = service_unit.clone();
            let handler = handler.clone();
            let vsidl_provider = vsidl_provider.clone();

            let dumpsys_logger_inner = dumpsys_logger.clone();
            tokio::spawn(async move {
                for _ in 0..20 {
                    let dt_stream = SubscriberStream::create(
                        context,
                        &service_unit.get_unit_identifier(),
                        Duration::ZERO,
                        SubscriberOptions { fetch_last_message: true },
                    )
                    .await;

                    if let Ok(mut dt_stream) = dt_stream {

                        while let Some(packet) = dt_stream.next().await {
                            match packet {
                                Ok((msg_count, buffer)) => {
                                    dumpsys_logger_inner.add_line(format!("Available to read: {msg_count}"));

                                    if msg_count != 0 {
                                        let size_for_one_msg = buffer.len() / msg_count as usize;

                                        let messages_bytes = buffer
                                            .chunks(size_for_one_msg)
                                            .map(|chunk| chunk.to_owned().into_boxed_slice())
                                            .collect::<Vec<_>>();

                                        for message_bytes in &messages_bytes {
                                            let event = decode_with_len::<T::Message>(message_bytes);

                                            if let Ok(event) = event {
                                                let handler = handler.clone();
                                                let service_unit = service_unit.clone();
                                                let vsidl_provider = vsidl_provider.clone();
                                                let dumpsys_logger_inner_clone = dumpsys_logger_inner.clone();
                                                let result = tokio::task::spawn_blocking(move || handler(service_unit, vsidl_provider, event, dumpsys_logger_inner_clone)).await;

                                                if result.is_err() {
                                                    info!("DEBUG: Error handling publication");
                                                    dumpsys_logger_inner.add_line("Error handling publication".to_string());
                                                }
                                            } else {
                                                dumpsys_logger_inner.add_line(format!("Failed to parse publication of type {} for {service_unit:?}", T::TYPE_NAME));
                                            }
                                        }
                                    }
                                },
                                Err(e) => {
                                    dumpsys_logger_inner.add_line(format!("Publisher unregistered: {e:#?}"))
                                }
                            }
                        }

                        break;
                    } else {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            });

            future::ready(())
        },
        dumpsys_logger_inner.clone(),
    )
    .await;
}

async fn listen_to_events(
    context: ContextRef,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
    dumpsys_logger: Arc<Logger>,
) {
    dumpsys_logger.add_line("Starting interaction with Events".to_owned());
    listen_to_all_publications_of_type::<EventPublication>(
        context,
        vsidl_provider,
        handle_event,
        dumpsys_logger.clone(),
    )
    .await
}

async fn listen_to_enable_conditions(
    context: ContextRef,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
    dumpsys_logger: Arc<Logger>,
) {
    dumpsys_logger.add_line("Starting interaction with EnableConditions".to_owned());
    listen_to_all_publications_of_type::<EnableConditionPublication>(
        context,
        vsidl_provider,
        handle_enable_condition,
        dumpsys_logger.clone(),
    )
    .await;
}

async fn listen_to_operation_cycles(
    context: ContextRef,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
    dumpsys_logger: Arc<Logger>,
) {
    dumpsys_logger.add_line("Starting interaction with OperationCycles".to_owned());
    listen_to_all_publications_of_type::<OperationCyclePublication>(
        context,
        vsidl_provider,
        handle_operation_cycle,
        dumpsys_logger.clone(),
    )
    .await;
}

#[instrument(skip_all)]
async fn read_data_items(
    context: ContextRef,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
    service_unit: ServiceUnitDefinition,
    dumpsys_logger: Arc<Logger>,
) {
    let service_fqin = service_unit.get_service_bundle_identity().get_fqin().clone();
    let translator = DiagnosticsTranslator::new(vsidl_provider.clone());

    let diagnostics_declaration = vsidl_provider
        .get_diagnostics_declaration(&service_fqin)
        .instrument(tracing::info_span!(
            "get_diagnostics_declaration_pre_read_data_rpc_call",
            fqin = service_unit.get_service_bundle_identity().get_fqin().get_service_bundle_name()
        ))
        .await
        .unwrap_or_default();

    dumpsys_logger.add_line(format!(
        "DataItems diagnostics declaration of a {} : {:#?}",
        service_fqin, diagnostics_declaration.data_item
    ));

    for data_item in diagnostics_declaration.data_item {
        dumpsys_logger.add_line(format!("Trying to read data item: {data_item:#?}"));
        let result = perform_and_log_rpc_call::<ReadDataItem>(
            context,
            &service_unit,
            ReadDataItemRequest { id: data_item.id.clone(), ..Default::default() },
            dumpsys_logger.clone(),
        )
        .instrument(tracing::info_span!(
            "read_data_item_rpc_call",
            fqbn = service_unit.get_service_bundle_identity().get_fqin().get_service_bundle_name(),
            data_type = service_unit.get_unit_identifier().get_unit_name(),
            data_item_id = data_item.id.clone()
        ))
        .await;

        let Ok(result) = result else {
            dumpsys_logger.add_line("Error: Failed to perform rpc call".to_owned());
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
                dumpsys_logger.add_line(format!(
                    "Read data item: {}#{}, proto_value: {:?}, uds_value {:?}",
                    &service_fqin, data_item.id, result.value, uds_data_item_value
                ));
            }
            Err(e) => {
                dumpsys_logger.add_line(format!("Error reading data item: {e}"));
            }
        };
    }
}

async fn print_typed_routine_start_response(
    vsidl_provider: &Arc<dyn VsidlConfigProviderV1>,
    diag_server_fqin: &ServiceFqin,
    routine_id: &str,
    response: &RoutineControlResponse,
    dumpsys_logger: Arc<Logger>,
) {
    let diagnostics_declaration =
        vsidl_provider.get_diagnostics_declaration(diag_server_fqin).await.unwrap_or_default();
    let routine = &diagnostics_declaration.routine.iter().find(|routine| routine.id == routine_id);

    let Some(routine) = routine else {
        warn!("Failed to retrieve routine info from declaration: {routine:?}");
        return;
    };

    let response_type_name = &routine.start.response;

    let response_descriptor =
        vsidl_provider.get_message_descriptor(diag_server_fqin, response_type_name).await.unwrap();
    let typed_routine_start_response =
        response_descriptor.parse_from_bytes(&response.response).unwrap();
    debug!(
        "(TEST RELEVANT LOG) For start method of routine: {routine_id}, typed response: {typed_routine_start_response:?}"
    );
    dumpsys_logger.add_line(format!(
        "(TEST RELEVANT LOG) For start method of routine: {routine_id}, typed response: {typed_routine_start_response:?}"
    ));
}

#[instrument(skip_all)]
async fn start_routines(
    context: ContextRef,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
    service_unit: ServiceUnitDefinition,
    dumpsys_logger: Arc<Logger>,
) {
    let service_fqin = service_unit.get_service_bundle_identity().get_fqin().clone();
    let diagnostics_declaration =
        vsidl_provider.get_diagnostics_declaration(&service_fqin).await.unwrap_or_default();
    let translator = DiagnosticsTranslator::new(vsidl_provider.clone());

    dumpsys_logger.add_line(format!(
        "RoutinControls diagnostics declaration of a {} : {:#?}",
        service_fqin, diagnostics_declaration.routine
    ));

    for routine in diagnostics_declaration.routine {
        dumpsys_logger.add_line(format!("Trying to start routine: {routine:#?}"));
        let response = perform_and_log_rpc_call::<StartRoutine>(
            context,
            &service_unit,
            RoutineControlRequest { id: routine.id.to_string(), ..Default::default() },
            dumpsys_logger.clone(),
        )
        .await;

        let Ok(response) = response else {
            dumpsys_logger.add_line("Error: Failed to perform rpc call".to_owned());
            continue;
        };

        print_typed_routine_start_response(
            &vsidl_provider,
            &service_fqin,
            &routine.id,
            &response,
            dumpsys_logger.clone(),
        )
        .await;

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
                dumpsys_logger.add_line(format!(
                    "Start routine: {}#{}, proto_value: {:?}, uds_value {:?}",
                    &service_fqin, routine.id, response.response, uds_response
                ));
            }
            Err(e) => {
                dumpsys_logger.add_line(format!("Error starting routine: {e}"));
            }
        };
    }
}

#[instrument(skip_all)]
async fn freeze_current_state(
    context: ContextRef,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
    service_unit: ServiceUnitDefinition,
    dumpsys_logger: Arc<Logger>,
) {
    let service_fqin = service_unit.get_service_bundle_identity().get_fqin().clone();
    let diagnostics_declaration =
        vsidl_provider.get_diagnostics_declaration(&service_fqin).await.unwrap_or_default();
    let translator = DiagnosticsTranslator::new(vsidl_provider.clone());

    dumpsys_logger.add_line(format!(
        "IoControls diagnostics declaration of a {} : {:#?}",
        service_fqin, diagnostics_declaration.io_control
    ));

    for io_control in diagnostics_declaration.io_control {
        dumpsys_logger.add_line(format!("Trying to freeze current state for: {io_control:#?}"));
        let response = perform_and_log_rpc_call::<FreezeCurrentState>(
            context,
            &service_unit,
            IoControlRequest { id: io_control.id.to_string(), ..Default::default() },
            dumpsys_logger.clone(),
        )
        .await;

        let Ok(response) = response else {
            dumpsys_logger.add_line("Error: Failed to perform rpc call".to_owned());
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
                dumpsys_logger.add_line(format!(
                    "Freeze current state for: {}#{}, proto_response: {:?}, uds_response: {:?}",
                    &service_fqin, io_control.id, response.value, uds_data_item_value
                ));
            }
            Err(e) => {
                dumpsys_logger.add_line(format!("Error freezing current state: {e}"));
            }
        };
    }
}

#[instrument(skip_all)]
async fn update_faults_status(
    context: ContextRef,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
    service_unit: ServiceUnitDefinition,
    dumpsys_logger: Arc<Logger>,
) {
    let service_fqin = service_unit.get_service_bundle_identity().get_fqin().clone();
    let diagnostics_declaration =
        vsidl_provider.get_diagnostics_declaration(&service_fqin).await.unwrap_or_default();

    dumpsys_logger.add_line(format!(
        "FaultListeners diagnostics declaration of a {} : {:#?}",
        service_fqin, diagnostics_declaration.event
    ));

    for event in diagnostics_declaration.event {
        dumpsys_logger.add_line(format!("Trying to update fault status: {event:#?}"));
        let response = perform_and_log_rpc_call::<OnFaultStatusChange>(
            context,
            &service_unit,
            OnFaultStatusChangeRequest {
                id: event.id.to_string(),
                status: 1,
                ..Default::default()
            },
            dumpsys_logger.clone(),
        )
        .await;

        if response.is_err() {
            dumpsys_logger.add_line("Error: Failed to perform rpc call".to_owned());
        };
    }
}

#[instrument(skip_all)]
fn handle_event(
    service_unit: ServiceUnitDefinition,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
    event: Event,
    dumpsys_logger: Arc<Logger>,
) {
    dumpsys_logger.add_line(format!("Event received: {event:#?}"));

    if let Some(ref extended_data) = event.extended_data {
        dumpsys_logger.add_line(format!("Event extended data received: {extended_data:#?}"));

        let unit_name = service_unit.get_unit_identifier().get_unit_name().to_string();

        let translator = DiagnosticsTranslator::new(vsidl_provider.clone());

        let service_fqin = service_unit.get_service_bundle_identity().get_fqin().clone();
        let diagnostics_declaration =
            futures_executor::block_on(vsidl_provider.get_diagnostics_declaration(&service_fqin))
                .unwrap_or_default();

        let event_declaration =
            diagnostics_declaration.event.iter().find(|event| event.service_unit_name == unit_name);

        if let Some(event_declaration) = event_declaration {
            dumpsys_logger.add_line(format!("Found declaration for event: {event_declaration}"));
            if let Some(data) = event.extended_data {
                let uds_extended_data =
                    futures_executor::block_on(translator.translate_from_protobuf_to_uds(
                        &service_fqin,
                        EntityId::EventExtendedData(event_declaration.id.clone()),
                        &data,
                    ));
                dumpsys_logger
                    .add_line(format!("Event extended data in UDS format: {uds_extended_data:#?}"));
            }
        } else {
            dumpsys_logger
                .add_line(format!("Didn't find declaration for event: {service_unit:#?}"));
        }
    }
}

#[instrument(skip_all)]
fn handle_enable_condition(
    _service_unit: ServiceUnitDefinition,
    _vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
    enable_condition: EnableCondition,
    dumpsys_logger: Arc<Logger>,
) {
    dumpsys_logger.add_line(format!("Enable condition change received: {enable_condition:#?}"));
}

fn handle_operation_cycle(
    _service_unit: ServiceUnitDefinition,
    _vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
    operation_cycle: OperationCycle,
    dumpsys_logger: Arc<Logger>,
) {
    dumpsys_logger.add_line(format!("Operaction cycle command received: {operation_cycle:#?}"));
}

async fn data_item_interaction(
    context: ContextRef,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
    dumpsys_logger: Arc<Logger>,
) {
    dumpsys_logger.add_line("Starting interaction with DataItems".to_owned());
    let dumpsys_logger_inner = dumpsys_logger.clone();

    set_service_unit_registration_callback(
        context,
        DataItem::unit_type(),
        move |service_unit| {
            tokio::spawn(read_data_items(
                context,
                vsidl_provider.clone(),
                service_unit,
                dumpsys_logger_inner.clone(),
            ));

            future::ready(())
        },
        dumpsys_logger.clone(),
    )
    .await;
}

#[instrument(skip_all)]
async fn io_control_interaction(
    context: ContextRef,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
    dumpsys_logger: Arc<Logger>,
) {
    dumpsys_logger.add_line("Starting interaction with IoControls".to_owned());
    let dumpsys_logger_inner = dumpsys_logger.clone();

    set_service_unit_registration_callback(
        context,
        IoControl::unit_type(),
        move |service_unit| {
            tokio::spawn(freeze_current_state(
                context,
                vsidl_provider.clone(),
                service_unit,
                dumpsys_logger_inner.clone(),
            ));

            future::ready(())
        },
        dumpsys_logger.clone(),
    )
    .await;
}

#[instrument(skip_all)]
async fn routine_control_interaction(
    context: ContextRef,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
    dumpsys_logger: Arc<Logger>,
) {
    dumpsys_logger.add_line("Starting interaction with RoutineControls".to_owned());
    let dumpsys_logger_inner = dumpsys_logger.clone();

    set_service_unit_registration_callback(
        context,
        RoutineControl::unit_type(),
        move |service_unit| {
            tokio::spawn(start_routines(
                context,
                vsidl_provider.clone(),
                service_unit,
                dumpsys_logger_inner.clone(),
            ));

            future::ready(())
        },
        dumpsys_logger.clone(),
    )
    .await;
}

#[instrument(skip_all)]
async fn fault_listener_interaction(
    context: ContextRef,
    vsidl_provider: Arc<dyn VsidlConfigProviderV1>,
    dumpsys_logger: Arc<Logger>,
) {
    dumpsys_logger.add_line("Starting interaction with FaultListeners".to_owned());
    let dumpsys_logger_inner = dumpsys_logger.clone();

    set_service_unit_registration_callback(
        context,
        FaultListener::unit_type(),
        move |service_unit| {
            tokio::spawn(update_faults_status(
                context,
                vsidl_provider.clone(),
                service_unit,
                dumpsys_logger_inner.clone(),
            ));

            future::ready(())
        },
        dumpsys_logger.clone(),
    )
    .await;
}

#[instrument(skip_all)]
async fn set_service_unit_registration_callback<F, Fut>(
    context: ContextRef,
    args: UnitType,
    f: F,
    dumpsys_logger: Arc<Logger>,
) where
    F: Fn(ServiceUnitDefinition) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    dumpsys_logger.add_line(format!("Listening for service units registration by type : {args}"));

    let sd = ServiceDiscoveryManager::new(context);

    let unit_stream = sd.subscribe_service_unit_change_by_type(&args).await.unwrap();

    let f = Arc::new(f);

    let dumpsys_logger_inner: Arc<Logger> = dumpsys_logger.clone();

    unit_stream
        .filter_map(move |event| {
            dumpsys_logger_inner.add_line(format!("Service unit change received: {event:#?}"));

            let result = if let ServiceUnitChangeEvent::Registered(unit) = event {
                dumpsys_logger_inner.add_line(format!(
                    "Service unit registered: {} / {}",
                    unit.get_service_bundle_identity().get_fqin(),
                    unit.get_unit_identifier().get_unit_name()
                ));
                Some(unit)
            } else {
                dumpsys_logger_inner.add_line("Ignoring unit Unregister/Change events".to_owned());
                None
            };

            future::ready(result)
        })
        .for_each(|unit: ServiceUnitDefinition| {
            dumpsys_logger.add_line(format!(
                "Discovered unit: {} : {}",
                unit.get_service_bundle_identity().get_fqin(),
                unit.get_unit_identifier().get_unit_name()
            ));

            tokio::spawn(f(unit.clone()));

            future::ready(())
        })
        .await
}

#[instrument(skip_all, fields(
    fqbn = service_unit.get_service_bundle_identity().get_fqin().get_service_bundle_name(),
    data_type = service_unit.get_unit_identifier().get_unit_name(),
    request = request.descriptor_dyn().name()
))]
async fn perform_and_log_rpc_call<T: Method>(
    context: ContextRef,
    service_unit: &ServiceUnitDefinition,
    request: T::Request,
    dumpsys_logger: Arc<Logger>,
) -> SdvResult<T::Response> {
    dumpsys_logger.add_line(format!(
        "Performing RPC call: {}:{}",
        T::UnitType::unit_type(),
        T::METHOD_NAME
    ));

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
    dumpsys_logger.add_line(format!(
        "Response for RPC call [{}]: {:?}",
        T::METHOD_NAME,
        response_decoded
    ));
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
    let size = input_stream.read_raw_varint32()?.into();
    // And parse only the part of the byte array that contains the message
    input_stream.push_limit(size)?;

    T::parse_from(&mut input_stream)
}

async fn create_tcp_connection() {
    let result = tokio::net::TcpListener::bind("127.0.0.1:8080").await;

    if result.is_err() {
        tokio::time::sleep(Duration::from_millis(10)).await;
        debug!("Retry creating TCP connection {result:#?}");
        Box::pin(create_tcp_connection()).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diagnostics_v1_api::event::Event;

    #[test]
    fn test_decode_with_len_buffer_out_of_bound_read() {
        let message_bytes = [0xff, 0xff];

        let result = decode_with_len::<Event>(&message_bytes);

        // Should result in error, instead of out-of-bound read and panic.
        assert!(result.is_err());
    }
}
