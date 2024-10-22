// Copyright 2024 Google LLC

use protobuf::Enum;
use rusqlite::Connection;
use sdv_status_rs::{SdvResult, SdvStatus, SdvStatusCode};
use std::sync::{Arc, Mutex};
use user_preferences_api::user::{user::UserFlags, User as UserProto};

use super::error::convert_rusqlite_error;

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct User {
    pub id: i32,
    pub flags: i32,
}

impl User {
    pub fn is_ephemeral(&self) -> bool {
        (self.flags & UserFlags::EPHEMERAL.value()) == UserFlags::EPHEMERAL.value()
    }
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

/// Provides to access to user data stored in a SQLite database
pub struct UserRepository {
    connection: Arc<Mutex<Connection>>,
}

impl UserRepository {
    pub fn new(connection: Arc<Mutex<Connection>>) -> SdvResult<Self> {
        connection
            .lock()
            .unwrap()
            .execute(
                "CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY,
                flags INTEGER,
                active BOOLEAN DEFAULT FALSE
            )",
                [],
            )
            .map_err(convert_rusqlite_error)?;
        Ok(UserRepository { connection })
    }

    /// Adds a new `user` to the database.
    ///
    /// Returns Err if user with the same id already exists or other failure happened.
    pub fn add_user(&self, user: &User) -> SdvResult<()> {
        let connection = self.connection.lock().unwrap();
        let changes = connection
            .execute(
                "INSERT INTO users (id, flags) VALUES (?, ?) ON CONFLICT(id) DO NOTHING",
                [&user.id, &user.flags],
            )
            .map_err(convert_rusqlite_error)?;

        // Check if no rows were changed (meaning the user already exists)
        if changes == 0 {
            return Err(SdvStatus::new(SdvStatusCode::AlreadyExists));
        }

        Ok(())
    }

    pub fn get_user(&self, id: i32) -> SdvResult<User> {
        let connection = self.connection.lock().unwrap();
        let mut stmt = connection
            .prepare("SELECT id, flags FROM users WHERE id = ?")
            .map_err(convert_rusqlite_error)?;
        let mut rows = stmt.query([id]).map_err(convert_rusqlite_error)?;

        if let Some(row) = rows.next().map_err(convert_rusqlite_error)? {
            let id = row.get(0).map_err(convert_rusqlite_error)?;
            let flags = row.get(1).map_err(convert_rusqlite_error)?;
            Ok(User { id, flags })
        } else {
            Err(SdvStatus::new(SdvStatusCode::NotFound))
        }
    }

    /// Deletes a user from the database.
    ///
    /// Returns Err if user with the given id does not exist or other failure happened.
    pub fn delete(&self, id: i32) -> SdvResult<()> {
        let connection = self.connection.lock().unwrap();
        let changes = connection
            .execute("DELETE FROM users WHERE id = ?", [&id])
            .map_err(convert_rusqlite_error)?;

        // Check if no rows were changed (meaning the user does not exist)
        if changes == 0 {
            return Err(SdvStatus::new(SdvStatusCode::NotFound));
        }

        Ok(())
    }

    pub fn list(&self) -> SdvResult<Vec<User>> {
        let connection = self.connection.lock().unwrap();
        let mut stmt =
            connection.prepare("SELECT id, flags FROM users").map_err(convert_rusqlite_error)?;
        let mut rows = stmt.query([]).map_err(convert_rusqlite_error)?;

        let mut users = Vec::new();
        while let Some(row) = rows.next().map_err(convert_rusqlite_error)? {
            let id = row.get(0).map_err(convert_rusqlite_error)?;
            let flags = row.get(1).map_err(convert_rusqlite_error)?;
            users.push(User { id, flags });
        }
        Ok(users)
    }

    /// Clears the entire database, removing all users.
    pub fn clear(&self) -> SdvResult<()> {
        let connection = self.connection.lock().unwrap();
        connection.execute("DELETE FROM users", []).map_err(convert_rusqlite_error)?;
        Ok(())
    }

    /// Sets the active user in the database.
    ///
    /// If another user is already active, this method will first deactivate them.
    /// Returns Err if the user with the given id does not exist or other failure happened.
    pub fn set_active_user(&self, id: i32) -> SdvResult<()> {
        let mut connection = self.connection.lock().unwrap();
        let tx = connection.transaction().map_err(convert_rusqlite_error)?;

        // Deactivate any previously active user
        tx.execute("UPDATE users SET active = FALSE WHERE active = TRUE", [])
            .map_err(convert_rusqlite_error)?;

        // Activate the specified user
        let changes = tx
            .execute("UPDATE users SET active = TRUE WHERE id = ?", [&id])
            .map_err(convert_rusqlite_error)?;

        if changes == 0 {
            return Err(SdvStatus::new(SdvStatusCode::NotFound));
        }

        tx.commit().map_err(convert_rusqlite_error)?;
        Ok(())
    }

    /// Returns the active user from the database.
    ///
    /// Returns Ok(None) if there is no active user.
    /// Returns Err if there is any error accessing the database.
    pub fn get_active_user(&self) -> SdvResult<Option<User>> {
        let connection = self.connection.lock().unwrap();
        let mut stmt = connection
            .prepare("SELECT id, flags FROM users WHERE active = TRUE")
            .map_err(convert_rusqlite_error)?;
        let mut rows = stmt.query([]).map_err(convert_rusqlite_error)?;

        match rows.next().map_err(convert_rusqlite_error)? {
            Some(row) => {
                let id = row.get(0).map_err(convert_rusqlite_error)?;
                let flags = row.get(1).map_err(convert_rusqlite_error)?;
                Ok(Some(User { id, flags }))
            }
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::assert_that;
    use googletest::prelude::*;

    #[test]
    fn add_user_already_exists() {
        let user_repository = create_in_memory_user_repository();

        // Add a user
        let user = User { id: 1, flags: 0 };
        user_repository.add_user(&user).unwrap();

        // Attempt to add the same user again
        let result = user_repository.add_user(&user);

        assert_that!(result, err(anything()));
        assert_eq!(result.unwrap_err().status_code(), SdvStatusCode::AlreadyExists);
    }

    #[test]
    fn get_user_returns_existing_user() {
        let user_repository = create_in_memory_user_repository();

        // Add a user
        let user = User { id: 1, flags: 0 };
        user_repository.add_user(&user).unwrap();

        // Retrieve the user
        let retrieved_user = user_repository.get_user(1).unwrap();

        // Assert that the retrieved user is the same as the added user
        assert_eq!(retrieved_user, user);
    }

    #[test]
    fn get_user_returns_error_for_non_existing_user() {
        let user_repository = create_in_memory_user_repository();

        // Attempt to retrieve a non-existing user
        let result = user_repository.get_user(100);

        // Assert that we get a NotFound Error

        assert_that!(result, err(anything()));
        assert_eq!(result.unwrap_err().status_code(), SdvStatusCode::NotFound);
    }

    #[test]
    fn delete_removes_existing_user() {
        let user_repository = create_in_memory_user_repository();

        let user = User { id: 1, flags: 0 };
        user_repository.add_user(&user).unwrap();

        user_repository.delete(1).unwrap();

        let result = user_repository.get_user(1);

        assert_that!(result, err(anything()));
        assert_eq!(result.unwrap_err().status_code(), SdvStatusCode::NotFound);
    }

    #[test]
    fn delete_returns_error_for_non_existing_user() {
        let user_repository = create_in_memory_user_repository();

        let result = user_repository.delete(100);

        assert_that!(result, err(anything()));
        assert_eq!(result.unwrap_err().status_code(), SdvStatusCode::NotFound);
    }

    #[test]
    fn list_existing_users() {
        let user_repository = create_in_memory_user_repository();

        let users = [User { id: 1, flags: 0 }, User { id: 2, flags: 1 }];

        for user in users.iter() {
            user_repository.add_user(user).unwrap();
        }

        let mut retrieved_users = user_repository.list().unwrap();

        retrieved_users.sort_by(|a, b| a.id.cmp(&b.id));

        assert_eq!(retrieved_users.len(), 2);
        assert_eq!(retrieved_users[0], User { id: 1, flags: 0 });
        assert_eq!(retrieved_users[1], User { id: 2, flags: 1 });
    }

    #[test]
    fn clear_removes_all_users() {
        let user_repository = create_in_memory_user_repository();

        let users = [User { id: 1, flags: 0 }, User { id: 2, flags: 1 }];
        for user in users.iter() {
            user_repository.add_user(user).unwrap();
        }

        user_repository.clear().unwrap();

        let retrieved_users = user_repository.list().unwrap();
        assert_eq!(retrieved_users.len(), 0);
    }

    #[test]
    fn user_proto_conversion() {
        let user = User { id: 1, flags: 2 };
        let user_proto: UserProto = user.into();

        assert_eq!(user_proto.id, 1);
        assert_eq!(user_proto.flags, 2);
    }

    #[test]
    fn set_active_user_sets_user_as_active() {
        let user_repository = create_in_memory_user_repository();

        // Add two users
        user_repository.add_user(&User { id: 1, flags: 0 }).unwrap();
        user_repository.add_user(&User { id: 2, flags: 0 }).unwrap();

        // Set user 2 as active
        user_repository.set_active_user(2).unwrap();

        // Retrieve the active user and verify it's user 2
        let active_user = user_repository.get_active_user().unwrap();
        assert_eq!(active_user, Some(User { id: 2, flags: 0 }));
    }

    #[test]
    fn set_active_user_deactivates_previous_user() {
        let user_repository = create_in_memory_user_repository();

        // Add two users
        user_repository.add_user(&User { id: 1, flags: 0 }).unwrap();
        user_repository.add_user(&User { id: 2, flags: 0 }).unwrap();

        // Set user 1 as active
        user_repository.set_active_user(1).unwrap();

        // Set user 2 as active
        user_repository.set_active_user(2).unwrap();

        // Retrieve the active user and verify it's user 2
        let active_user = user_repository.get_active_user().unwrap();
        assert_eq!(active_user, Some(User { id: 2, flags: 0 }));
    }

    #[test]
    fn set_active_user_returns_error_for_non_existing_user() {
        let user_repository = create_in_memory_user_repository();

        let result = user_repository.set_active_user(100);

        assert_that!(result, err(anything()));
        assert_eq!(result.unwrap_err().status_code(), SdvStatusCode::NotFound);
    }

    #[test]
    fn get_active_user_returns_none_when_no_active_user() {
        let user_repository = create_in_memory_user_repository();

        let active_user = user_repository.get_active_user().unwrap();

        assert_eq!(active_user, None);
    }

    #[test]
    fn get_active_user_returns_the_active_user() {
        let user_repository = create_in_memory_user_repository();

        user_repository.add_user(&User { id: 1, flags: 0 }).unwrap();
        user_repository.set_active_user(1).unwrap();

        let active_user = user_repository.get_active_user().unwrap();

        assert_eq!(active_user, Some(User { id: 1, flags: 0 }));
    }

    fn create_in_memory_user_repository() -> UserRepository {
        let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
        UserRepository::new(connection).unwrap()
    }
}
