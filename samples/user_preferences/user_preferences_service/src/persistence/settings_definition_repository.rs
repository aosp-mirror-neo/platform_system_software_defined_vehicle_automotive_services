use rusqlite::Connection;
use serde_json::to_string;
use std::sync::{Arc, Mutex};

use super::error::Error;
use super::setting_definition::SettingDefinition;

/// Provides access to the list setting definitions stored in a SQLITE database.
/// Persisting these settings is important to ensure clients of User Preferences are able
/// to access settings before the User Controllable Service is started.
pub struct SettingDefinitionRepository {
    connection: Arc<Mutex<Connection>>,
}

impl SettingDefinitionRepository {
    pub fn new(connection: Arc<Mutex<Connection>>) -> Result<Self, Error> {
        // Create the table if it doesn't exist
        connection.lock().unwrap().execute(
            "CREATE TABLE IF NOT EXISTS setting_definition (
                    settings_group_id INTEGER,
                    setting_name TEXT NOT NULL,
                    value TEXT NOT NULL,
                    kind INTEGER NOT NULL,
                    constraints TEXT,
                    PRIMARY KEY (settings_group_id, setting_name),
                    FOREIGN KEY (settings_group_id) REFERENCES settings_groups(id)
                );",
            [],
        )?;
        Ok(SettingDefinitionRepository { connection })
    }

    pub fn store_all(
        &self,
        settings_group_id: i64,
        setting_definitions: &[SettingDefinition],
    ) -> Result<(), Error> {
        let mut connection = self.connection.lock().unwrap();
        let tx = connection.transaction()?;

        for setting_definition in setting_definitions {
            let constraints_json = match &setting_definition.constraints {
                Some(constraints) => to_string(constraints)?,
                None => "null".to_string(),
            };

            tx.execute(
                "INSERT OR REPLACE INTO setting_definition (settings_group_id, setting_name, value, kind, constraints) VALUES (?, ?, ?, ?, ?)",
                (
                    settings_group_id,
                    &setting_definition.name,
                    &to_string(&setting_definition.default_value)?,
                    setting_definition.kind,
                    &constraints_json,
                ),
            )?;
        }

        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::constraint::{Constraint, Int32Constraint};
    use crate::persistence::setting::Value;

    #[test]
    fn store_all_new_settings() {
        let repo = create_in_memory_setting_definition_repository();

        let settings_group_id = 1;
        let setting_definitions = vec![
            SettingDefinition {
                kind: 1,
                name: "bool_setting".to_string(),
                default_value: Value::Bool(true),
                constraints: None,
            },
            SettingDefinition {
                kind: 2,
                name: "int32_setting".to_string(),
                default_value: Value::Int32(10),
                constraints: Some(Constraint::Int32(Int32Constraint {
                    min_value: Some(0),
                    max_value: Some(20),
                    step: Some(1),
                })),
            },
        ];

        repo.store_all(settings_group_id, &setting_definitions).unwrap();
    }

    #[test]
    fn store_all_update_existing_settings() {
        let repo = create_in_memory_setting_definition_repository();

        let settings_group_id = 1;
        let initial_setting_definitions = vec![
            SettingDefinition {
                kind: 1,
                name: "bool_setting".to_string(),
                default_value: Value::Bool(false),
                constraints: None,
            },
            SettingDefinition {
                kind: 2,
                name: "int32_setting".to_string(),
                default_value: Value::Int32(5),
                constraints: Some(Constraint::Int32(Int32Constraint {
                    min_value: Some(0),
                    max_value: Some(10),
                    step: Some(1),
                })),
            },
        ];

        repo.store_all(settings_group_id, &initial_setting_definitions).unwrap();

        let updated_setting_definitions = vec![
            SettingDefinition {
                kind: 1,
                name: "bool_setting".to_string(),
                default_value: Value::Bool(true),
                constraints: None,
            },
            SettingDefinition {
                kind: 2,
                name: "int32_setting".to_string(),
                default_value: Value::Int32(15),
                constraints: Some(Constraint::Int32(Int32Constraint {
                    min_value: Some(10),
                    max_value: Some(30),
                    step: Some(5),
                })),
            },
        ];

        repo.store_all(settings_group_id, &updated_setting_definitions).unwrap();
    }

    fn create_in_memory_setting_definition_repository() -> SettingDefinitionRepository {
        let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
        SettingDefinitionRepository::new(connection).unwrap()
    }
}
