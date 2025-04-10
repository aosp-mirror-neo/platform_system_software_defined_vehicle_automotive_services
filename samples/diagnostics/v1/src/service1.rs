// Copyright 2024 Google LLC

#![allow(missing_docs)]

use diagnostics_v1_sample_catalog::messages::*;
use sdv::comms::id::ServiceFqin;
use sdv::comms::ContextRef;
use sdv::diagnostics::api::response_code::ResponseCode;
use sdv::diagnostics::{
    data_item::*, fault_listener::*, io_control::*, routine::*, DiagnosticsResult,
};
use sdv::mw::SdvComms;
use sdv_dump::DumpBinderConnection;
use sdv_mw_rs_com_sdv_oem_sample_diagnostics_diagnostics_provider1::{
    diagnostics::Diagnostics, DiagnosticsProvider1,
};
use std::str::FromStr;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use protobuf::well_known_types::empty::Empty;
use sdv::diagnostics::api::event::Status;

struct PressureDataItem;
#[async_trait::async_trait]
impl ReadOnlyDataItemHandler<Pressure> for PressureDataItem {
    async fn read(&self) -> DiagnosticsResult<Pressure> {
        println!("Read Pressure DataItem called");
        Ok(Pressure { value: 100.1f32, ..Default::default() })
    }
}

struct LowPressureThresholdDataItem;
#[async_trait::async_trait]
impl DataItemHandler<Pressure> for LowPressureThresholdDataItem {
    async fn read(&self) -> DiagnosticsResult<Pressure> {
        println!("Read LowPressureThreshold DataItem called");
        Ok(Pressure { value: 100.1f32, ..Default::default() })
    }

    async fn write(&self, _: Pressure) -> ResponseCode {
        println!("Write LowPressureThreshold DataItem called: unimplemented");
        ResponseCode::GENERAL_REJECT
    }
}

struct PressurizationRoutine;
#[async_trait::async_trait]
impl RoutineHandler<Pressure, Empty, Empty, Empty, Empty, Empty> for PressurizationRoutine {
    async fn start(&self, pressure: Pressure) -> RoutineResult<Empty> {
        println!("Start PressurizationRoutine called, pressure [{}]", pressure);
        Ok(RoutineResponse::<Empty>::default())
    }
}

struct PressureIoControl;
#[async_trait::async_trait]
impl IoControlHandler<Pressure> for PressureIoControl {
    async fn return_control_to_ecu(&self, _mask: &[u8]) -> DiagnosticsResult<Pressure> {
        todo!();
    }
    async fn reset_to_default(&self, _mask: &[u8]) -> DiagnosticsResult<Pressure> {
        todo!();
    }
    async fn freeze_current_state(&self, _mask: &[u8]) -> DiagnosticsResult<Pressure> {
        println!("Freeze current state for PressureIoControl called");
        Ok(Pressure { value: 100.1f32, ..Default::default() })
    }
    async fn short_term_adjustment(
        &self,
        _adjusted_value: Pressure,
        _mask: &[u8],
    ) -> DiagnosticsResult<Pressure> {
        todo!();
    }
}

struct LowPressureFault;
#[async_trait::async_trait]
impl FaultListenerHandler<LowPressureData> for LowPressureFault {
    async fn on_fault_status_change(&self, status: u8, data: Option<LowPressureData>) {
        println!(
            "On fault status change for LowPressureFault called: status [{}], data [{:?}]",
            status, data
        )
    }
}

static BINDER_TAG: &str = "diagnostics_test";
static DUMP_REPORT_TITLE: &str = "SDV Sample Dump - Diagnostics Test";

#[tokio::main]
async fn main() {
    // Implementing dump() for the sample to be able to reliably monitor it from automated tests
    let dm = DumpBinderConnection::initialize(DUMP_REPORT_TITLE, BINDER_TAG);

    let ctx: ContextRef = ContextRef::create(
        ServiceFqin::from_str("vm:com.sdv.oem.sample.diagnostics.DiagnosticsProvider1/instance")
            .unwrap(),
    );
    let sdv_comms = SdvComms { context: ctx };
    dm.set_self_fqin(&ctx.get_self_fqin());

    let service = DiagnosticsProvider1::new(
        Arc::new(sdv_comms),
        (
            Diagnostics::create_data_item_server(
                Arc::new(PressureDataItem {}),
                Arc::new(LowPressureThresholdDataItem {}),
            ),
            Diagnostics::create_routine_control_server(Arc::new(PressurizationRoutine {})),
            Diagnostics::create_io_control_server(Arc::new(PressureIoControl {})),
            Diagnostics::create_fault_listener_server(Arc::new(LowPressureFault {})),
        ),
    )
    .await
    .unwrap();

    println!("Diagnostics provider service bundle started.");

    let result_pass = service.diagnostics.lowpressure_event_publisher.publish(Status::PASS);
    println!("{result_pass:#?}");
    thread::sleep(Duration::from_secs(1));
    let result_pre_fail = service.diagnostics.lowpressure_event_publisher.publish(Status::PRE_FAIL);
    println!("{result_pre_fail:#?}");

    if result_pass.is_ok() && result_pre_fail.is_ok() {
        dm.set_agent_state("Diagnostics Test Ok");
    } else {
        dm.set_agent_state("Diagnostics Test Error");
    }

    sdv_dump::ProcessState::join_thread_pool();
}
