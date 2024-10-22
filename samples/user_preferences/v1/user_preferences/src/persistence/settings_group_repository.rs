// Copyright 2024 Google LLC

use rusqlite::Connection;
use sdv_status_rs::{SdvResult, SdvStatus, SdvStatusCode};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::{Arc, Mutex};
use user_preferences_api::settings_group::SettingsGroupId as SettingsGroupIdProto;

use super::error::convert_rusqlite_error;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SettingsGroup {
    // Should not be set in store operations, this ID is auto generated
    pub id: i64,
    pub name: String,
    pub service_fqin: String,
}

impl fmt::Display for SettingsGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.service_fqin, self.name)
    }
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
    pub fn new(connection: Arc<Mutex<Connection>>) -> SdvResult<Self> {
        // Create the table if it doesn't exist
        connection
            .lock()
            .unwrap()
            .execute(
                "CREATE TABLE IF NOT EXISTS settings_groups (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT,
                service_fqin TEXT,
                UNIQUE(name, service_fqin)
            )",
                [],
            )
            .map_err(convert_rusqlite_error)?;
        Ok(SettingsGroupRepository { connection })
    }

    pub fn get(&self, name: &str, service_fqin: &str) -> SdvResult<SettingsGroup> {
        let connection = self.connection.lock().unwrap();
        let mut stmt = connection.prepare(
            "SELECT id, name, service_fqin FROM settings_groups WHERE name = ? AND service_fqin = ?",
        ).map_err(convert_rusqlite_error)?;
        let mut rows = stmt.query((&name, &service_fqin)).map_err(convert_rusqlite_error)?;

        if let Some(row) = rows.next().map_err(convert_rusqlite_error)? {
            let id = row.get(0).map_err(convert_rusqlite_error)?;
            let name = row.get(1).map_err(convert_rusqlite_error)?;
            let service_fqin = row.get(2).map_err(convert_rusqlite_error)?;
            Ok(SettingsGroup { id, name, service_fqin })
        } else {
            Err(SdvStatus::new(SdvStatusCode::NotFound))
        }
    }

    /// Stores a new settings group in the database.
    ///
    /// Returns err if setting group already exists or other failure happened
    pub fn store(&self, settings_group: &SettingsGroup) -> SdvResult<i64> {
        let connection = self.connection.lock().unwrap();

        // Insert the new settings group, only if it doesn't already exist
        connection.execute(
            "INSERT INTO settings_groups (name, service_fqin) VALUES (?, ?) ON CONFLICT(name, service_fqin) DO NOTHING",
            (&settings_group.name, &settings_group.service_fqin),
        ).map_err(convert_rusqlite_error)?;

        // Retrieve the ID of the inserted settings group
        Ok(connection.last_insert_rowid())
    }

    pub fn list(&self) -> SdvResult<Vec<SettingsGroup>> {
        let connection = self.connection.lock().unwrap();
        let mut stmt = connection
            .prepare("SELECT id, name, service_fqin FROM settings_groups")
            .map_err(convert_rusqlite_error)?;
        let mut rows = stmt.query([]).map_err(convert_rusqlite_error)?;

        let mut settings_groups = Vec::new();
        while let Some(row) = rows.next().map_err(convert_rusqlite_error)? {
            let id = row.get(0).map_err(convert_rusqlite_error)?;
            let name = row.get(1).map_err(convert_rusqlite_error)?;
            let service_fqin = row.get(2).map_err(convert_rusqlite_error)?;
            settings_groups.push(SettingsGroup { id, name, service_fqin });
        }
        Ok(settings_groups)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::assert_that;
    use googletest::prelude::*;

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

    #[test]
    fn get_existing_settings_group() {
        let settings_group_repository = create_in_memory_settings_group_repository();

        let settings_group = SettingsGroup {
            name: "test_group".to_string(),
            service_fqin: "com.example.test_service".to_string(),
            id: 0,
        };
        // Store the settings group
        let settings_group_id = settings_group_repository.store(&settings_group).unwrap();

        // Retrieve the settings group
        let retrieved_settings_group = settings_group_repository
            .get(&settings_group.name, &settings_group.service_fqin)
            .unwrap();

        assert_eq!(retrieved_settings_group.id, settings_group_id);
        assert_eq!(retrieved_settings_group.name, settings_group.name);
        assert_eq!(retrieved_settings_group.service_fqin, settings_group.service_fqin);
    }

    #[test]
    fn get_non_existing_settings_group_should_return_not_found_error() {
        let settings_group_repository = create_in_memory_settings_group_repository();

        let name = "non_existing_group";
        let service_fqin = "com.example.non_existing_service";

        // Retrieve the settings group (should return None)
        let result = settings_group_repository.get(name, service_fqin);

        assert_that!(result, err(anything()));
        assert_eq!(result.unwrap_err().status_code(), SdvStatusCode::NotFound);
    }

    #[test]
    fn list_existing_settings_groups() {
        let settings_group_repository = create_in_memory_settings_group_repository();

        let settings_groups = [
            SettingsGroup {
                id: 1,
                name: "group1".to_string(),
                service_fqin: "com.example.group1".to_string(),
            },
            SettingsGroup {
                id: 2,
                name: "group2".to_string(),
                service_fqin: "com.example.group2".to_string(),
            },
        ];

        for settings_group in settings_groups.iter() {
            settings_group_repository.store(settings_group).unwrap();
        }

        let retrieved_settings_groups = settings_group_repository.list().unwrap();

        assert_eq!(retrieved_settings_groups.len(), 2);

        // Check the first settings group
        assert_eq!(
            retrieved_settings_groups[0],
            SettingsGroup {
                id: 1,
                name: "group1".to_string(),
                service_fqin: "com.example.group1".to_string(),
            }
        );

        // Check the second settings group
        assert_eq!(
            retrieved_settings_groups[1],
            SettingsGroup {
                id: 2,
                name: "group2".to_string(),
                service_fqin: "com.example.group2".to_string(),
            }
        );
    }

    fn create_in_memory_settings_group_repository() -> SettingsGroupRepository {
        let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
        SettingsGroupRepository::new(connection).unwrap()
    }
}
