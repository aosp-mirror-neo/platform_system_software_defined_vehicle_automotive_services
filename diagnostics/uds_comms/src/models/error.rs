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
