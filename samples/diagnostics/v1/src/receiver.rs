// Copyright 2024 Google LLC

#![allow(missing_docs, unused_imports)]

use sdv_comms::id::ServiceFqin;
use sdv_comms::ContextRef;
use sdv_mw_rs_comms::codec::decode;
use sdv_mw_rs_comms::Communicate;
use sdv_mw_rs_comms::{Publication, SubscribeOptions, Subscriber, UnitName, UnitQuery};
use sdv_mw_rs_sdv_comms::SdvComms;
use std::str::FromStr;
use std::sync::Arc;

use diagnostics_v1_api::event::Event;
use diagnostics_v1_sample_catalog::messages::LowPressureData;

#[tokio::main]
async fn main() {
    run().await.unwrap();
}

async fn run() -> anyhow::Result<()> {
    let sdv_comms = SdvComms {
        context: ContextRef::create(
            ServiceFqin::from_str("vm:sdv.diagnostics.sample.DiagnosticsReceiver/sample").unwrap(),
        ),
    };

    let publication = sdv_comms
        .find_publication(&UnitQuery::Name(UnitName {
            vm: "".to_string(),
            package: "sdv.diagnostics.sample".to_string(),
            bundle: "DiagnosticsProvider1".to_string(),
            name: "sdv-diagnostics-event-lowpressure".to_string(),
        }))
        .await;

    println!("Publication created");

    let subscriber = Subscriber::<Event>::new(
        publication?,
        &SubscribeOptions { read_on_subscribe: false, ..Default::default() },
    )
    .await;

    println!("Subscriber created");

    let _ = subscriber?.read_next_messages(1)?.iter().inspect(|message| {
        println!("Received event: {message:#?} with status {:#?}", message.status);

        let decoded_data =
            message.extended_data.as_ref().map(|data| decode::<LowPressureData>(data));
        println!("Event data: {decoded_data:#?}");
    });

    loop {
        std::thread::park();
    }
}
