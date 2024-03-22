// Copyright 2024 Google LLC

use diagnostics_api::event::Status;

/// Rust representatuin of diagnostics event.
pub struct Event<T> {
    /// Event's status (PASS, FAIL, etc.).
    pub status: Status,
    /// Optional extended data attached to the event status update.
    pub extended_data: Option<T>,
}

impl<T> Event<T> {
    /// Creates new instance of the event.
    pub fn new(status: Status, extended_data: Option<T>) -> Self {
        Self { status, extended_data }
    }
}
