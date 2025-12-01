// Copyright 2025 Google LLC

use crate::doip::codec::DoipCodecError;
use crate::doip::error::DoipTransportError;
use crate::models::request::UdsRequestTranslationError;
use crate::models::response::UdsResponseTranslationError;

/// Represents all possible errors for the UDS communications library.
#[derive(Debug, thiserror::Error)]
pub enum UdsCommsError {
    /// An underlying DoIP transport-layer error occurred.
    #[error("DoIP transport error: {0}")]
    DoipTransport(#[from] DoipTransportError),

    /// An error occurred when applying the DoIP codec to a message.
    #[error("DoIP codec error: {0}.")]
    DoipCodec(#[from] DoipCodecError),

    /// An error occurred during UDS request translation.
    #[error("UDS request translation error: {0}")]
    UdsRequestTranslation(#[from] UdsRequestTranslationError),

    /// An error occurred during UDS response translation.
    #[error("UDS response translation error: {0}")]
    UdsResponseTranslation(#[from] UdsResponseTranslationError),
}
