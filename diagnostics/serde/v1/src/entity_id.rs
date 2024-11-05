// Copyright 2024 Google LLC

/// Diagnostics entity id.
#[derive(Debug)]
pub enum EntityId {
    /// Diagnostics data item with certain id.
    DataItem(String),
    /// Diagnostics input/output control data with certain id.
    InputOutputData(String),
    /// Routine communication data.
    Routine {
        /// Routine id.
        id: String,
        /// Routine method.
        method: RoutineMethod,
        /// Method's message.
        message: MethodMessage,
    },
    /// Extended data of the diagnostics event.
    EventExtendedData(String),
}

/// Routine control method.
#[derive(Debug)]
pub enum RoutineMethod {
    /// See com.sdv.google.diagnostics.routine_control.RoutineControl::Start.
    Start,
    /// See com.sdv.google.diagnostics.routine_control.RoutineControl::Stop.
    Stop,
    /// See com.sdv.google.diagnostics.routine_control.RoutineControl::RequestResult.
    RequestResult,
}

/// Routine control method message.
#[derive(Debug)]
pub enum MethodMessage {
    /// Request of the routine control method.
    Request,
    /// Response of the routine control method.
    Response,
}
