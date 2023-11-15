// Copyright 2023 Google LLC
#![allow(unused_variables)]

use crate::not_implemented;
use android_sdv_diagnostics::aidl::android::sdv::diagnostics::{
    data_item::{DataItemDescription::DataItemDescription, IDataItemHandler::IDataItemHandler},
    event::{EventStatus::EventStatus, IFaultListener::IFaultListener},
    io_control::{
        IIOControlHandler::IIOControlHandler, IOControlDescription::IOControlDescription,
    },
    routine_control::{
        IRoutineControlHandler::IRoutineControlHandler,
        RoutineControlDescription::RoutineControlDescription,
    },
    IDiagnosticsManager::{IDiagnosticsManager, OperationCycleCommand::OperationCycleCommand},
    ResponseCode::ResponseCode,
};
use android_sdv_diagnostics::binder::{self, Interface, Result, Strong};

pub struct DiagnosticsManager;
impl Interface for DiagnosticsManager {}
impl IDiagnosticsManager for DiagnosticsManager {
    fn setEventStatus(&self, id: &str, status: EventStatus) -> Result<()> {
        not_implemented!()
    }

    fn registerFaultListener(
        &self,
        ids: &[String],
        listener: &Strong<dyn IFaultListener>,
    ) -> Result<()> {
        not_implemented!()
    }

    fn setEnableCondition(&self, id: &str, fulfilled: bool) -> Result<()> {
        not_implemented!()
    }

    fn setOperationCycle(&self, id: &str, command: OperationCycleCommand) -> Result<()> {
        not_implemented!()
    }

    fn registerDataItemHandler(
        &self,
        descriptions: &[DataItemDescription],
        handler: &Strong<dyn IDataItemHandler>,
    ) -> Result<()> {
        not_implemented!()
    }

    fn registerRoutineControlHandler(
        &self,
        descriptions: &[RoutineControlDescription],
        handler: &Strong<dyn IRoutineControlHandler>,
    ) -> Result<()> {
        not_implemented!()
    }

    fn registerIOControlHandler(
        &self,
        descriptions: &[IOControlDescription],
        handler: &Strong<dyn IIOControlHandler>,
    ) -> Result<()> {
        not_implemented!()
    }

    fn registerServiceConfiguration(&self, configuration: &[u8]) -> Result<()> {
        not_implemented!()
    }
}
