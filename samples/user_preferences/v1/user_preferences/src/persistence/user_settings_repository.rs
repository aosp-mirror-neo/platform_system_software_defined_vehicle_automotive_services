// Copyright 2024 Google LLC

use super::setting::{Setting, Value};
use crate::persistence::error::{convert_rusqlite_error, convert_serialization_error};
use rusqlite::Connection;
use sdv_status_rs::SdvResult;
use serde_json::{from_str, to_string};
use std::sync::{Arc, Mutex};

pub struct UserSettingsRepository {
    connection: Arc<Mutex<Connection>>,
}

impl UserSettingsRepository {
    pub fn new(connection: Arc<Mutex<Connection>>) -> SdvResult<Self> {
        // Create the table if it doesn't exist
        connection
            .lock()
            .unwrap()
            .execute(
                "CREATE TABLE IF NOT EXISTS user_settings (
                user_id INTEGER,
                settings_group_id INTEGER,
                setting_name TEXT,
                value TEXT,
                PRIMARY KEY (user_id, settings_group_id, setting_name),
                FOREIGN KEY (settings_group_id) REFERENCES settings_groups(id)
            )",
                [],
            )
            .map_err(convert_rusqlite_error)?;
        Ok(UserSettingsRepository { connection })
    }

    pub fn get_all(&self, user_id: i32, settings_group_id: i64) -> SdvResult<Vec<Setting>> {
        let connection = self.connection.lock().unwrap();
        let mut stmt = connection.prepare(
            "SELECT setting_name, value FROM user_settings WHERE user_id = ? AND settings_group_id = ?",
        ).map_err(convert_rusqlite_error)?;
        let mut rows =
            stmt.query((&user_id, &settings_group_id)).map_err(convert_rusqlite_error)?;

        let mut settings = Vec::new();
        while let Some(row) = rows.next().map_err(convert_rusqlite_error)? {
            let setting_name = row.get(0).map_err(convert_rusqlite_error)?;
            let value_json: String = row.get(1).map_err(convert_rusqlite_error)?;

            let value: Value = from_str(&value_json).map_err(convert_serialization_error)?;

            settings.push(Setting { name: setting_name, value });
        }
        Ok(settings)
    }

    pub fn store_all(
        &self,
        user_id: i32,
        settings_group_id: i64,
        settings: &[Setting],
    ) -> SdvResult<()> {
        let mut connection = self.connection.lock().unwrap();

        // Start a transaction for atomicity
        let tx = connection.transaction().map_err(convert_rusqlite_error)?;

        for setting in settings {
            let value_json = to_string(&setting.value).map_err(convert_serialization_error)?;
            tx.execute(
                "INSERT OR REPLACE INTO user_settings (user_id, settings_group_id, setting_name, value) VALUES (?, ?, ?, ?)",
                (
                    &user_id,
                    &settings_group_id,
                    &setting.name,
                    &value_json,
                ),
            ).map_err(convert_rusqlite_error)?;
        }

        tx.commit().map_err(convert_rusqlite_error)?;
        Ok(())
    }

    /// Deletes all rows associated with a specific user ID.
    pub fn delete_all(&self, user_id: i32) -> SdvResult<()> {
        let connection = self.connection.lock().unwrap();
        connection
            .execute("DELETE FROM user_settings WHERE user_id = ?", [&user_id])
            .map_err(convert_rusqlite_error)?;
        Ok(())
    }

    /// Clears the entire database, removing all user settings.
    pub fn clear(&self) -> SdvResult<()> {
        let connection = self.connection.lock().unwrap();
        connection.execute("DELETE FROM user_settings", []).map_err(convert_rusqlite_error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_all_new_settings() {
        let repo = create_in_memory_user_settings_repository();

        let user_id = 1;
        let settings_group_id = 1;
        let settings = vec![
            Setting { name: "bool_setting".to_string(), value: Value::Bool(true) },
            Setting { name: "int32_setting".to_string(), value: Value::Int32(10) },
        ];

        repo.store_all(user_id, settings_group_id, &settings).unwrap();
    }

    #[test]
    fn store_all_update_existing_settings() {
        let repo = create_in_memory_user_settings_repository();

        let user_id = 1;
        let settings_group_id = 1;
        let initial_settings = vec![
            Setting { name: "bool_setting".to_string(), value: Value::Bool(false) },
            Setting { name: "int32_setting".to_string(), value: Value::Int32(5) },
        ];

        repo.store_all(user_id, settings_group_id, &initial_settings).unwrap();

        let updated_settings = vec![
            Setting { name: "bool_setting".to_string(), value: Value::Bool(true) },
            Setting { name: "int32_setting".to_string(), value: Value::Int32(15) },
        ];

        repo.store_all(user_id, settings_group_id, &updated_settings).unwrap();
    }

    #[test]
    fn get_all_existing_settings() {
        let repo = create_in_memory_user_settings_repository();

        let user_id = 1;
        let settings_group_id = 1;
        let settings = vec![
            Setting { name: "bool_setting".to_string(), value: Value::Bool(true) },
            Setting { name: "int32_setting".to_string(), value: Value::Int32(10) },
        ];

        repo.store_all(user_id, settings_group_id, &settings).unwrap();

        let retrieved_settings = repo.get_all(user_id, settings_group_id).unwrap();

        assert_eq!(retrieved_settings.len(), 2);

        // Check the first setting
        assert_eq!(retrieved_settings[0].name, "bool_setting");
        assert_eq!(retrieved_settings[0].value, Value::Bool(true));

        // Check the second setting
        assert_eq!(retrieved_settings[1].name, "int32_setting");
        assert_eq!(retrieved_settings[1].value, Value::Int32(10));
    }

    #[test]
    fn get_all_no_settings() {
        let repo = create_in_memory_user_settings_repository();

        let user_id = 1;
        let settings_group_id = 1;

        let retrieved_settings = repo.get_all(user_id, settings_group_id).unwrap();

        assert_eq!(retrieved_settings.len(), 0);
    }

    #[test]
    fn delete_all_removes_all_settings_for_user() {
        let repo = create_in_memory_user_settings_repository();

        let user_id = 1;
        let settings_group_id = 1;
        let settings = vec![
            Setting { name: "bool_setting".to_string(), value: Value::Bool(true) },
            Setting { name: "int32_setting".to_string(), value: Value::Int32(10) },
        ];

        repo.store_all(user_id, settings_group_id, &settings).unwrap();

        repo.delete_all(user_id).unwrap();

        let retrieved_settings = repo.get_all(user_id, settings_group_id).unwrap();
        assert_eq!(retrieved_settings.len(), 0);
    }

    #[test]
    fn clear_removes_all_settings() {
        let repo = create_in_memory_user_settings_repository();

        let user_id = 1;
        let settings_group_id = 1;
        let settings = vec![
            Setting { name: "bool_setting".to_string(), value: Value::Bool(true) },
            Setting { name: "int32_setting".to_string(), value: Value::Int32(10) },
        ];

        repo.store_all(user_id, settings_group_id, &settings).unwrap();

        repo.clear().unwrap();

        let retrieved_settings = repo.get_all(user_id, settings_group_id).unwrap();
        assert_eq!(retrieved_settings.len(), 0);
    }

    fn create_in_memory_user_settings_repository() -> UserSettingsRepository {
        let connection = Connection::open_in_memory().unwrap();
        UserSettingsRepository::new(Arc::new(Mutex::new(connection))).unwrap()
    }
}
