#![allow(missing_docs, unused_imports)]
// Copyright 2023 Google LLC

use diagnostics_api::event::Status;
use diagnostics_api::negative_response_code::NegativeResponseCode;
use diagnostics_sample_catalog::messages::{
    DataItemOne, FirstEventExt, RoutineRequestParameter, RoutineResponseParameter,
};
use sdv_middleware_extension_diagnostics::data_item::DataItemHandler;
use sdv_middleware_extension_diagnostics::event::Event;
use sdv_middleware_extension_diagnostics::routine::{
    RoutineHandler, RoutineResponse, RoutineResult,
};
use sdv_middleware_extension_diagnostics::DiagnosticsResult;
use sdvgenerated::diagnostics_provider_service1::DiagnosticsProviderService1;
use std::sync::Mutex;

struct DataItemOneHandler {
    value: Mutex<DataItemOne>,
}

impl DataItemOneHandler {
    fn new(initial_value: DataItemOne) -> Self {
        Self { value: Mutex::new(initial_value) }
    }
}

impl DataItemHandler<DataItemOne> for DataItemOneHandler {
    fn read(&self) -> DiagnosticsResult<DataItemOne> {
        Ok((*self.value.lock().unwrap()).clone())
    }

    fn write(&self, value: DataItemOne) -> NegativeResponseCode {
        *self.value.lock().unwrap() = value;
        NegativeResponseCode::POSITIVE_RESPONSE
    }
}

struct Routine1Handler {}

impl
    RoutineHandler<
        RoutineRequestParameter,
        RoutineResponseParameter,
        RoutineRequestParameter,
        RoutineResponseParameter,
        RoutineRequestParameter,
        RoutineResponseParameter,
    > for Routine1Handler
{
    fn start(&self, req: RoutineRequestParameter) -> RoutineResult<RoutineResponseParameter> {
        Ok(RoutineResponse {
            response: RoutineResponseParameter {
                echo: format!("start echo: {}", req.value),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    fn stop(&self, req: RoutineRequestParameter) -> RoutineResult<RoutineResponseParameter> {
        Ok(RoutineResponse {
            response: RoutineResponseParameter {
                echo: format!("stop echo: {}", req.value),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    fn request_result(
        &self,
        _: RoutineRequestParameter,
    ) -> RoutineResult<RoutineResponseParameter> {
        Err(NegativeResponseCode::BUSY_REPEAT_REQUEST)
    }
}

fn main() {
    let mut service = DiagnosticsProviderService1::new();

    // Setup data item server.
    let data_item_server_impl =
        service.diagnostics.create_data_item_server(Box::new(DataItemOneHandler::new(
            DataItemOne { code: 1, name: "hey".to_string(), enabled: true, ..Default::default() },
        )));

    service.data_item_server.register_service_interface(data_item_server_impl);

    // Setup routine server.
    let routine_server_impl =
        service.diagnostics.create_routine_server(Box::new(Routine1Handler {}));

    service.routine_control_server.register_service_interface(routine_server_impl);

    service.start().unwrap();

    println!("Diagnostics provider service bundle started.");

    service
        .diagnostics
        .publish_event1_event(Event::new(
            Status::PRE_FAIL,
            Some(FirstEventExt { ..Default::default() }),
        ))
        .unwrap();

    service.diagnostics.publish_event1_event(Event::new(Status::PRE_PASS, None)).unwrap();

    loop {
        std::thread::park();
    }
}
