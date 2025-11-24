// Copyright 2025 Google LLC

use crate::models::request::UdsRequestTranslationError;
use crate::models::response::UdsResponseTranslationError;

/// Represents all possible errors for the UDS communications library.
#[derive(Debug, thiserror::Error)]
pub enum UdsCommsError {
    /// An underlying transport-layer error occurred.
    #[error("Transport error: {0}")]
    Transport(#[from] std::io::Error),

    /// An error occurred during UDS request translation.
    #[error("UDS request translation error: {0}")]
    UdsRequestTranslation(#[from] UdsRequestTranslationError),

    /// An error occurred during UDS response translation.
    #[error("UDS response translation error: {0}")]
    UdsResponseTranslation(#[from] UdsResponseTranslationError),
}
