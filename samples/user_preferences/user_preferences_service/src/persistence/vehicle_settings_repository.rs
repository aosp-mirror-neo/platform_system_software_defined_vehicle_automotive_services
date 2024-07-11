use anyhow::Result;
use rusqlite::Connection;
use serde_json::to_string;
use std::sync::{Arc, Mutex};

use super::setting::Setting;

/// Provides access to the list of vehicle settings stored in a SQLite database.
/// This repository persists the settings currently applied to the vehicle,
/// allowing clients to retrieve them even if the User Controllable Service is offline.
pub struct VehicleSettingsRepository {
    connection: Arc<Mutex<Connection>>,
}

impl VehicleSettingsRepository {
    pub fn new(connection: Arc<Mutex<Connection>>) -> Result<Self> {
        // Create the table if it doesn't exist
        connection.lock().unwrap().execute(
            "CREATE TABLE IF NOT EXISTS vehicle_settings (
                settings_group_id INTEGER,
                setting_name TEXT,
                value TEXT,
                PRIMARY KEY (settings_group_id, setting_name),
                FOREIGN KEY (settings_group_id) REFERENCES settings_groups(id)
            )",
            [],
        )?;
        Ok(VehicleSettingsRepository { connection })
    }

    /// Stores a list of vehicle settings for a specific settings group.
    ///
    /// This method updates the database with the provided settings, replacing any existing
    /// settings for the given settings group.
    pub fn store_all(&self, settings_group_id: i64, settings: Vec<Setting>) -> Result<()> {
        let connection = self.connection.lock().unwrap();

        for setting in settings {
            let value_json = to_string(&setting.value)?;
            connection.execute(
                "INSERT OR REPLACE INTO vehicle_settings (settings_group_id, setting_name, value) VALUES (?, ?, ?)",
                (
                    &settings_group_id,
                    &setting.name,
                    &value_json,
                ),
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::setting::{Setting, Value};

    #[test]
    fn store_all_new_settings() {
        let repo = create_in_memory_vehicle_settings_repository();

        let settings_group_id = 1;
        let settings = vec![
            Setting { name: "bool_setting".to_string(), value: Value::Bool(true) },
            Setting { name: "int32_setting".to_string(), value: Value::Int32(10) },
        ];

        repo.store_all(settings_group_id, settings.clone()).unwrap();
    }

    #[test]
    fn store_all_update_existing_settings() {
        let repo = create_in_memory_vehicle_settings_repository();

        let settings_group_id = 1;
        let initial_settings = vec![
            Setting { name: "bool_setting".to_string(), value: Value::Bool(false) },
            Setting { name: "int32_setting".to_string(), value: Value::Int32(5) },
        ];

        repo.store_all(settings_group_id, initial_settings.clone()).unwrap();

        let updated_settings = vec![
            Setting { name: "bool_setting".to_string(), value: Value::Bool(true) },
            Setting { name: "int32_setting".to_string(), value: Value::Int32(15) },
        ];

        repo.store_all(settings_group_id, updated_settings.clone()).unwrap();
    }

    fn create_in_memory_vehicle_settings_repository() -> VehicleSettingsRepository {
        let connection = Connection::open_in_memory().unwrap();
        VehicleSettingsRepository::new(Arc::new(Mutex::new(connection))).unwrap()
    }
}
