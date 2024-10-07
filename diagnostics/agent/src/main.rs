// Copyright 2024 Google LLC

//! Diagnostics agent implementation.

use diagnostics_v1_api::event::Event;
use futures::stream::StreamExt;
use log::info;
use protobuf::Message;
use sdv_comms::dt::{Subscriber, SubscriberOptions};
use sdv_comms::id::ServiceFqin;
use sdv_comms::sd::ServiceDiscoveryManager;
use sdv_comms::sd::UnitType;
use sdv_comms::ContextRef;
use std::collections::HashSet;
use std::str::FromStr;
use std::thread;
use std::time::Duration;
use google_sdv_rpc_aidl::aidl::google::sdv::rpc::IRpcAgent::{BpRpcAgent, IRpcAgent};
use google_sdv_service_discovery_discovery_aidl::aidl::google::sdv::service_discovery::discovery::IServiceDiscoveryAgent::{BpServiceDiscoveryAgent, IServiceDiscoveryAgent};

#[tokio::main]
async fn main() {
    sdv_log::init_logger("sdv_diagnostics_agent").unwrap();
    sdv_tracing::init_tracing();

    wait_for_comms_stack();

    let self_fqin =
        ServiceFqin::from_str("vm:sdv.google.diagnostics.DiagnosticsAgent/default").unwrap();
    let context = ContextRef::create(self_fqin.clone());

    info!("Starting diagnostics: {self_fqin:#?}");

    listen_to_events(context).await;

    binder::ProcessState::start_thread_pool();

    binder::ProcessState::join_thread_pool();
}

async fn listen_to_events(context: ContextRef) {
    let mut event_subscribers = HashSet::new();
    let sd = ServiceDiscoveryManager::new(context);

    loop {
        let mut service_units = sd.list_units_by_type(
            &UnitType::from_str("sdv.diagnostics.DiagnosticsManager#EventEvent").unwrap(),
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
                let event = decode::<Event>(message_bytes);
                info!("Event received: {:#?}", event);

                if let Some(ref extended_data) = event.extended_data {
                    // TODO(372607401): use VsidlProvider to parse the extended data message.
                    info!("Event extended data received: {:#?}", extended_data);
                }
            }
        }

        thread::sleep(Duration::from_secs(1));
    }
}

fn wait_for_comms_stack() {
    let descriptor = <BpServiceDiscoveryAgent as IServiceDiscoveryAgent>::get_descriptor()
        .to_owned()
        + "/default";
    with_retry(|| binder::wait_for_interface::<dyn IServiceDiscoveryAgent>(&descriptor));

    let descriptor = <BpRpcAgent as IRpcAgent>::get_descriptor().to_owned() + "/default";
    with_retry(|| binder::wait_for_interface::<dyn IRpcAgent>(&descriptor));
}

fn with_retry<T, E, F>(function: F) -> T
where
    F: Fn() -> Result<T, E>,
{
    function().unwrap_or_else(|_| with_retry(function))
}

fn decode<T: Message>(message_bytes: &[u8]) -> T {
    let mut input_stream = protobuf::CodedInputStream::from_bytes(message_bytes);
    let size = input_stream.read_raw_varint32().unwrap().into();
    // And parse only the part of the byte array that contains the message
    let _ = input_stream.push_limit(size);

    T::parse_from(&mut input_stream).unwrap()
}
