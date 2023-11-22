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
    IDiagnosticsManager::{
        IDiagnosticsManagerAsyncServer, OperationCycleCommand::OperationCycleCommand,
    },
    ResponseCode::ResponseCode,
};
use android_sdv_diagnostics::binder::{self, Interface, Result, Strong};

pub struct DiagnosticsManagerAsync;
impl Interface for DiagnosticsManagerAsync {}

#[::async_trait::async_trait]
impl IDiagnosticsManagerAsyncServer for DiagnosticsManagerAsync {
    async fn setEventStatus(&self, id: &str, status: EventStatus) -> Result<()> {
        not_implemented!()
    }

    async fn registerFaultListener(
        &self,
        ids: &[String],
        listener: &Strong<dyn IFaultListener>,
    ) -> Result<()> {
        not_implemented!()
    }

    async fn setEnableCondition(&self, id: &str, fulfilled: bool) -> Result<()> {
        not_implemented!()
    }

    async fn setOperationCycle(&self, id: &str, command: OperationCycleCommand) -> Result<()> {
        not_implemented!()
    }

    async fn registerDataItemHandler(
        &self,
        descriptions: &[DataItemDescription],
        handler: &Strong<dyn IDataItemHandler>,
    ) -> Result<()> {
        not_implemented!()
    }

    async fn registerRoutineControlHandler(
        &self,
        descriptions: &[RoutineControlDescription],
        handler: &Strong<dyn IRoutineControlHandler>,
    ) -> Result<()> {
        not_implemented!()
    }

    async fn registerIOControlHandler(
        &self,
        descriptions: &[IOControlDescription],
        handler: &Strong<dyn IIOControlHandler>,
    ) -> Result<()> {
        not_implemented!()
    }

    async fn registerServiceConfiguration(&self, configuration: &[u8]) -> Result<()> {
        not_implemented!()
    }
}
