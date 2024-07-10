use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use user_preferences_api::settings_group::SettingsGroupId as SettingsGroupIdProto;

use super::error::Error;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SettingsGroup {
    // Should not be set in store operations, this ID is auto generated
    pub id: i64,
    pub name: String,
    pub service_fqin: String,
}

impl From<SettingsGroup> for SettingsGroupIdProto {
    fn from(value: SettingsGroup) -> Self {
        SettingsGroupIdProto {
            service_fqin: value.service_fqin,
            name: value.name,
            ..Default::default()
        }
    }
}

pub struct SettingsGroupRepository {
    connection: Arc<Mutex<Connection>>,
}

/// Provides access to list of Setting Groups stored in a SQLite database
impl SettingsGroupRepository {
    pub fn new(connection: Arc<Mutex<Connection>>) -> Result<Self, Error> {
        // Create the table if it doesn't exist
        connection.lock().unwrap().execute(
            "CREATE TABLE IF NOT EXISTS settings_groups (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT,
                service_fqin TEXT,
                UNIQUE(name, service_fqin)
            )",
            [],
        )?;
        Ok(SettingsGroupRepository { connection })
    }

    /// Stores a new settings group in the database.
    ///
    /// Returns err if setting group already exists or other failure happened
    pub fn store(&self, settings_group: &SettingsGroup) -> Result<i64, Error> {
        let connection = self.connection.lock().unwrap();

        // Insert the new settings group, only if it doesn't already exist
        connection.execute(
            "INSERT INTO settings_groups (name, service_fqin) VALUES (?, ?) ON CONFLICT(name, service_fqin) DO NOTHING",
            (&settings_group.name, &settings_group.service_fqin),
        )?;

        // Retrieve the ID of the inserted settings group
        Ok(connection.last_insert_rowid())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_new_settings_group() {
        let settings_group_repository = create_in_memory_settings_group_repository();

        let settings_group = SettingsGroup {
            id: 0, // ID is ignored here
            name: "test_group".to_string(),
            service_fqin: "com.example.test_service".to_string(),
        };

        let id = settings_group_repository.store(&settings_group).unwrap();

        assert!(id > 0);
    }

    #[test]
    fn store_existing_settings_group_returns_id_of_existing_group() {
        let settings_group_repository = create_in_memory_settings_group_repository();

        let settings_group = SettingsGroup {
            id: 0, // ID is ignored here
            name: "test_group".to_string(),
            service_fqin: "com.example.test_service".to_string(),
        };

        // Store the settings group once
        let id1 = settings_group_repository.store(&settings_group).unwrap();

        // Store it again (should not create a new entry)
        let id2 = settings_group_repository.store(&settings_group).unwrap();

        assert_eq!(id1, id2);
    }

    fn create_in_memory_settings_group_repository() -> SettingsGroupRepository {
        let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
        SettingsGroupRepository::new(connection).unwrap()
    }
}
