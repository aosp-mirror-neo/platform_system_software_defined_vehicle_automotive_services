// Copyright 2024 Google LLC

use rusqlite::{Error as RusqliteError, ErrorCode as RusqliteErrorCode};
use serde_json::Error as SerdeJsonError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Object not found")]
    NotFound,

    #[error("Object already exists")]
    AlreadyExists,

    #[error("Unable to serialize object: {0}")]
    #[allow(clippy::enum_variant_names)]
    SerializationError(SerdeJsonError),

    #[error("Database error: {0}")]
    #[allow(clippy::enum_variant_names)]
    DatabaseError(RusqliteError),
}

// Custom From implementation for RusqliteError
impl From<RusqliteError> for Error {
    fn from(err: RusqliteError) -> Self {
        match err {
            RusqliteError::QueryReturnedNoRows => Self::NotFound,
            RusqliteError::SqliteFailure(sqlite_err, _) => {
                if sqlite_err.code == RusqliteErrorCode::ConstraintViolation {
                    Self::AlreadyExists
                } else {
                    Self::DatabaseError(err)
                }
            }
            _ => Self::DatabaseError(err),
        }
    }
}

// Custom From implementation for SerdeJsonError
impl From<SerdeJsonError> for Error {
    fn from(err: SerdeJsonError) -> Self {
        Self::SerializationError(err)
    }
}
