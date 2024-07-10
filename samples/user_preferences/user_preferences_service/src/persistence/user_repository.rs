use rusqlite::{Connection, Error as RusqliteError, ErrorCode as RusqliteErrorCode};
use std::sync::{Arc, Mutex};
use thiserror::Error;
use user_preferences_api::user::User as UserProto;

#[derive(Debug, PartialEq)]
pub struct User {
    pub id: i32,
    pub flags: i32,
}

impl From<User> for UserProto {
    fn from(value: User) -> Self {
        Self { id: value.id, flags: value.flags, ..Default::default() }
    }
}

impl From<UserProto> for User {
    fn from(value: UserProto) -> Self {
        Self { id: value.id, flags: value.flags }
    }
}

#[derive(Error, Debug, PartialEq)]
pub enum UserRepositoryError {
    #[error("User not found")]
    NotFound,

    #[error("User already exists")]
    AlreadyExists,

    #[error("Database error: {0}")]
    DatabaseError(RusqliteError),
}

// Custom From implementation for RusqliteError
impl From<RusqliteError> for UserRepositoryError {
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

/// Provides to access to user data stored in a SQLite database
pub struct UserRepository {
    connection: Arc<Mutex<Connection>>,
}

impl UserRepository {
    pub fn new(connection: Arc<Mutex<Connection>>) -> Result<Self, UserRepositoryError> {
        connection.lock().unwrap().execute(
            "CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY,
                flags INTEGER
            )",
            [],
        )?;
        Ok(UserRepository { connection })
    }

    /// Adds a new `user` to the database.
    ///
    /// Returns Err if user with the same id already exists or other failure happened.
    pub fn add_user(&self, user: &User) -> Result<(), UserRepositoryError> {
        let connection = self.connection.lock().unwrap();
        let changes = connection.execute(
            "INSERT INTO users (id, flags) VALUES (?, ?) ON CONFLICT(id) DO NOTHING",
            [&user.id, &user.flags],
        )?;

        // Check if no rows were changed (meaning the user already exists)
        if changes == 0 {
            return Err(UserRepositoryError::AlreadyExists);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_user_already_exists() {
        let user_repository = create_in_memory_user_repository();

        // Add a user
        let user = User { id: 1, flags: 0 };
        user_repository.add_user(&user).unwrap();

        // Attempt to add the same user again
        let result = user_repository.add_user(&user);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), UserRepositoryError::AlreadyExists);
    }

    #[test]
    fn user_proto_conversion() {
        let user = User { id: 1, flags: 2 };
        let user_proto: UserProto = user.into();

        assert_eq!(user_proto.id, 1);
        assert_eq!(user_proto.flags, 2);
    }

    fn create_in_memory_user_repository() -> UserRepository {
        let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
        UserRepository::new(connection).unwrap()
    }
}
