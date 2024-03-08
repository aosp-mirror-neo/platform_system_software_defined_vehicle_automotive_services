// Copyright 2024 Google LLC

/// Diagnostics entity id.
#[derive(Debug)]
pub enum EntityId {
    /// Diagnostics data item with certain id.
    DataItem(String),
}
