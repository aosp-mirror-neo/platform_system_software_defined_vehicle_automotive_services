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
