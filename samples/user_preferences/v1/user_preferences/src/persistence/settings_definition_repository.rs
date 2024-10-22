// Copyright 2024 Google LLC

use rusqlite::Connection;
use sdv_status_rs::SdvResult;
use serde_json::{from_str, to_string};
use std::sync::{Arc, Mutex};

use super::constraint::*;
use super::error::{convert_rusqlite_error, convert_serialization_error};
use super::setting::Value;
use super::setting_definition::SettingDefinition;

/// Provides access to the list setting definitions stored in a SQLITE database.
/// Persisting these settings is important to ensure clients of User Preferences are able
/// to access settings before the User Controllable Service is started.
pub struct SettingDefinitionRepository {
    connection: Arc<Mutex<Connection>>,
}

impl SettingDefinitionRepository {
    pub fn new(connection: Arc<Mutex<Connection>>) -> SdvResult<Self> {
        // Create the table if it doesn't exist
        connection
            .lock()
            .unwrap()
            .execute(
                "CREATE TABLE IF NOT EXISTS setting_definition (
                    settings_group_id INTEGER,
                    setting_name TEXT NOT NULL,
                    default_value TEXT NOT NULL,
                    kind INTEGER NOT NULL,
                    constraints TEXT,
                    PRIMARY KEY (settings_group_id, setting_name),
                    FOREIGN KEY (settings_group_id) REFERENCES settings_groups(id)
                );",
                [],
            )
            .map_err(convert_rusqlite_error)?;
        Ok(SettingDefinitionRepository { connection })
    }

    pub fn store(
        &self,
        settings_group_id: i64,
        setting_definitions: &[SettingDefinition],
    ) -> SdvResult<()> {
        let mut connection = self.connection.lock().unwrap();
        let tx = connection.transaction().map_err(convert_rusqlite_error)?;

        for setting_definition in setting_definitions {
            let constraints_json = match &setting_definition.constraint {
                Some(constraints) => {
                    Some(to_string(constraints).map_err(convert_serialization_error)?)
                }
                None => None,
            };

            tx.execute(
                "INSERT OR REPLACE INTO setting_definition (settings_group_id, setting_name, default_value, kind, constraints) VALUES (?, ?, ?, ?, ?)",
                (
                    settings_group_id,
                    &setting_definition.name,
                    &to_string(&setting_definition.default_value).map_err(convert_serialization_error)?,
                    setting_definition.kind,
                    &constraints_json,
                ),
            ).map_err(convert_rusqlite_error)?;
        }

        tx.commit().map_err(convert_rusqlite_error)?;
        Ok(())
    }

    pub fn get(&self, settings_group_id: i64) -> SdvResult<Vec<SettingDefinition>> {
        let connection = self.connection.lock().unwrap();
        let mut stmt = connection.prepare(
            "SELECT setting_name, default_value, kind, constraints FROM setting_definition WHERE settings_group_id = ?",
        ).map_err(convert_rusqlite_error)?;
        let mut rows = stmt.query([settings_group_id]).map_err(convert_rusqlite_error)?;

        let mut setting_definitions = Vec::new();
        while let Some(row) = rows.next().map_err(convert_rusqlite_error)? {
            let setting_name: String = row.get(0).map_err(convert_rusqlite_error)?;
            let default_value_json: String = row.get(1).map_err(convert_rusqlite_error)?;
            let kind: i32 = row.get(2).map_err(convert_rusqlite_error)?;
            let constraint_json: Option<String> = row.get(3).map_err(convert_rusqlite_error)?;

            let default_value: Value =
                from_str(default_value_json.as_str()).map_err(convert_serialization_error)?;
            let constraints: Option<Constraint> = match constraint_json {
                Some(x) => Some(from_str(x.as_str()).map_err(convert_serialization_error)?),
                None => None,
            };

            setting_definitions.push(SettingDefinition {
                name: setting_name,
                default_value,
                kind,
                constraint: constraints,
            });
        }

        Ok(setting_definitions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::constraint::{Constraint, Int32Constraint};
    use crate::persistence::setting::Value;

    #[test]
    fn store_new_settings() {
        let repo = create_in_memory_setting_definition_repository();

        let settings_group_id = 1;
        let setting_definitions = vec![
            SettingDefinition {
                kind: 1,
                name: "bool_setting".to_string(),
                default_value: Value::Bool(true),
                constraint: None,
            },
            SettingDefinition {
                kind: 2,
                name: "int32_setting".to_string(),
                default_value: Value::Int32(10),
                constraint: Some(Constraint::Int32(Int32Constraint {
                    min_value: Some(0),
                    max_value: Some(20),
                    step: Some(1),
                })),
            },
        ];

        repo.store(settings_group_id, &setting_definitions).unwrap();
    }

    #[test]
    fn store_update_existing_settings() {
        let repo = create_in_memory_setting_definition_repository();

        let settings_group_id = 1;
        let initial_setting_definitions = vec![
            SettingDefinition {
                kind: 1,
                name: "bool_setting".to_string(),
                default_value: Value::Bool(false),
                constraint: None,
            },
            SettingDefinition {
                kind: 2,
                name: "int32_setting".to_string(),
                default_value: Value::Int32(5),
                constraint: Some(Constraint::Int32(Int32Constraint {
                    min_value: Some(0),
                    max_value: Some(10),
                    step: Some(1),
                })),
            },
        ];

        repo.store(settings_group_id, &initial_setting_definitions).unwrap();

        let updated_setting_definitions = vec![
            SettingDefinition {
                kind: 1,
                name: "bool_setting".to_string(),
                default_value: Value::Bool(true),
                constraint: None,
            },
            SettingDefinition {
                kind: 2,
                name: "int32_setting".to_string(),
                default_value: Value::Int32(15),
                constraint: Some(Constraint::Int32(Int32Constraint {
                    min_value: Some(10),
                    max_value: Some(30),
                    step: Some(5),
                })),
            },
        ];

        repo.store(settings_group_id, &updated_setting_definitions).unwrap();
    }

    #[test]
    fn get_existing_settings() {
        let repo = create_in_memory_setting_definition_repository();

        let settings_group_id = 1_i64;
        let setting_definitions = vec![
            SettingDefinition {
                kind: 1,
                name: "bool_setting".to_string(),
                default_value: Value::Bool(true),
                constraint: None,
            },
            SettingDefinition {
                kind: 2,
                name: "int32_setting".to_string(),
                default_value: Value::Int32(10),
                constraint: Some(Constraint::Int32(Int32Constraint {
                    min_value: Some(0),
                    max_value: Some(20),
                    step: Some(1),
                })),
            },
        ];

        repo.store(settings_group_id, &setting_definitions).unwrap();

        let retrieved_settings = repo.get(settings_group_id).unwrap();

        assert_eq!(retrieved_settings.len(), 2);

        // Check the first setting
        assert_eq!(retrieved_settings[0].kind, 1);
        assert_eq!(retrieved_settings[0].name, "bool_setting");
        assert_eq!(retrieved_settings[0].default_value, Value::Bool(true));
        assert_eq!(retrieved_settings[0].constraint, None);

        // Check the second setting
        assert_eq!(retrieved_settings[1].kind, 2);
        assert_eq!(retrieved_settings[1].name, "int32_setting");
        assert_eq!(retrieved_settings[1].default_value, Value::Int32(10));
        assert_eq!(
            retrieved_settings[1].constraint,
            Some(Constraint::Int32(Int32Constraint {
                min_value: Some(0),
                max_value: Some(20),
                step: Some(1),
            }))
        );
    }

    fn create_in_memory_setting_definition_repository() -> SettingDefinitionRepository {
        let connection = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
        SettingDefinitionRepository::new(connection).unwrap()
    }
}
