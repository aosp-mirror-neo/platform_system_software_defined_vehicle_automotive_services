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

use diagnostics_v1_api::{
    data_item_service::*, fault_listener_service::*, io_control_service::*,
    routine_control_service::*,
};
use protobuf::MessageFull;
use sdv_comms::sd::UnitType;

pub trait DiagnosticsUnitType {
    const TYPE_NAME: &'static str;
    fn unit_type() -> UnitType {
        UnitType::builder()
            .set_sdv_package_name("com.sdv.google.diagnostics")
            .set_service_bundle_name("DiagnosticsManager")
            .set_unit_type_name(Self::TYPE_NAME)
            .build()
            .unwrap_or_else(|e| {
                panic!("Failed to create Diagnostics UnitType for {}: {e}", Self::TYPE_NAME)
            })
    }
}

pub trait PublicationUnitType: DiagnosticsUnitType {
    type Message: MessageFull;
}

pub trait Method {
    const METHOD_NAME: &'static str;
    type Request: MessageFull;
    type Response: MessageFull;
    type UnitType: DiagnosticsUnitType;
}

pub struct DataItem;
impl DiagnosticsUnitType for DataItem {
    const TYPE_NAME: &'static str = "data_item.DataItemService";
}

pub struct ReadDataItem;
impl Method for ReadDataItem {
    const METHOD_NAME: &'static str = "ReadDataItem";
    type Request = ReadDataItemRequest;
    type Response = ReadDataItemResponse;
    type UnitType = DataItem;
}

pub struct FaultListener;
impl DiagnosticsUnitType for FaultListener {
    const TYPE_NAME: &'static str = "fault.FaultListenerService";
}

pub struct OnFaultStatusChange;
impl Method for OnFaultStatusChange {
    const METHOD_NAME: &'static str = "OnFaultStatusChange";
    type Request = OnFaultStatusChangeRequest;
    type Response = OnFaultStatusChangeResponse;
    type UnitType = FaultListener;
}

pub struct RoutineControl;
impl DiagnosticsUnitType for RoutineControl {
    const TYPE_NAME: &'static str = "routine_control.RoutineControlService";
}

pub struct StartRoutine;
impl Method for StartRoutine {
    const METHOD_NAME: &'static str = "StartRoutine";
    type Request = RoutineControlRequest;
    type Response = RoutineControlResponse;
    type UnitType = RoutineControl;
}

pub struct IoControl;
impl DiagnosticsUnitType for IoControl {
    const TYPE_NAME: &'static str = "io_control.IoControlService";
}

pub struct FreezeCurrentState;
impl Method for FreezeCurrentState {
    const METHOD_NAME: &'static str = "FreezeCurrentState";
    type Request = IoControlRequest;
    type Response = IoControlResponse;
    type UnitType = IoControl;
}

pub struct EventPublication;
impl DiagnosticsUnitType for EventPublication {
    const TYPE_NAME: &'static str = "event.Event";
}

impl PublicationUnitType for EventPublication {
    type Message = diagnostics_v1_api::event::Event;
}

pub struct OperationCyclePublication;
impl DiagnosticsUnitType for OperationCyclePublication {
    const TYPE_NAME: &'static str = "event.OperationCycle";
}

impl PublicationUnitType for OperationCyclePublication {
    type Message = diagnostics_v1_api::event::OperationCycle;
}

pub struct EnableConditionPublication;
impl DiagnosticsUnitType for EnableConditionPublication {
    const TYPE_NAME: &'static str = "event.EnableCondition";
}

impl PublicationUnitType for EnableConditionPublication {
    type Message = diagnostics_v1_api::event::EnableCondition;
}
