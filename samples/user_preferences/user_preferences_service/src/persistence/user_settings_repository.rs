use crate::persistence::error::Error;
use rusqlite::Connection;
use serde_json::from_str;
use std::sync::{Arc, Mutex};

use super::setting::{Setting, Value};

pub struct UserSettingsRepository {
    connection: Arc<Mutex<Connection>>,
}

impl UserSettingsRepository {
    pub fn new(connection: Arc<Mutex<Connection>>) -> Result<Self, Error> {
        // Create the table if it doesn't exist
        connection.lock().unwrap().execute(
            "CREATE TABLE IF NOT EXISTS user_settings (
                user_id INTEGER,
                settings_group_id INTEGER,
                setting_name TEXT,
                value TEXT,
                PRIMARY KEY (user_id, settings_group_id, setting_name),
                FOREIGN KEY (settings_group_id) REFERENCES settings_groups(id)
            )",
            [],
        )?;
        Ok(UserSettingsRepository { connection })
    }

    pub fn get_all(&self, user_id: i32, settings_group_id: i64) -> Result<Vec<Setting>, Error> {
        let connection = self.connection.lock().unwrap();
        let mut stmt = connection.prepare(
            "SELECT setting_name, value FROM user_settings WHERE user_id = ? AND settings_group_id = ?",
        )?;
        let mut rows = stmt.query((&user_id, &settings_group_id))?;

        let mut settings = Vec::new();
        while let Some(row) = rows.next()? {
            let setting_name = row.get(0)?;
            let value_json: String = row.get(1)?;

            let value: Value = from_str(&value_json)?;

            settings.push(Setting { name: setting_name, value });
        }
        Ok(settings)
    }
}
