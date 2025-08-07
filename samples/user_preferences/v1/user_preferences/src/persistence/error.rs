// Copyright 2024 Google LLC

use rusqlite::{Error as RusqliteError, ErrorCode as RusqliteErrorCode};
use sdv_status_rs::{SdvStatus, SdvStatusCode};
use serde_json::Error as SerdeJsonError;

pub fn convert_rusqlite_error(err: RusqliteError) -> SdvStatus {
    match err {
        // Access the inner RusqliteError
        RusqliteError::QueryReturnedNoRows => SdvStatus::new(SdvStatusCode::NotFound),
        RusqliteError::SqliteFailure(sqlite_err, _) => {
            if sqlite_err.code == RusqliteErrorCode::ConstraintViolation {
                SdvStatus::new(SdvStatusCode::AlreadyExists)
            } else {
                SdvStatus::with_message(SdvStatusCode::Internal, format!("{err}"))
            }
        }
        _ => SdvStatus::with_message(SdvStatusCode::Internal, format!("{err}")),
    }
}

pub fn convert_serialization_error(err: SerdeJsonError) -> SdvStatus {
    SdvStatus::with_message(
        SdvStatusCode::Internal,
        format!("Failed to serialize/deserialize: {err:?}"),
    )
}
