use protobuf::Enum;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::vec::Vec;

use crate::settings_store::SettingsStore;
use user_preferences_api::error::Error;
use user_preferences_api::error::ResponseCode;
use user_preferences_api::user::{user::UserFlags, User as UserProto};

#[derive(Clone)]
pub struct User {
    pub id: i32,
    pub flags: i32,
    pub vehicle_configuration: Arc<Mutex<SettingsStore>>,
}

impl User {
    fn new(id: i32, flags: i32, vehicle_configuration: SettingsStore) -> Self {
        Self { id, flags, vehicle_configuration: Arc::new(Mutex::new(vehicle_configuration)) }
    }

    pub fn is_ephemeral(&self) -> bool {
        (self.flags & UserFlags::EPHEMERAL.value()) == UserFlags::EPHEMERAL.value()
    }
}

pub struct UserManager {
    users: Mutex<HashMap<i32, Arc<User>>>,
}

impl UserManager {
    pub fn new() -> Self {
        Self { users: Mutex::new(HashMap::new()) }
    }

    pub fn create_user(
        &self,
        user: UserProto,
        vehicle_configuration: SettingsStore,
    ) -> Result<(), Error> {
        if !Self::are_valid_user_flags(user.flags) {
            println!("Invalid user flags set");
            return Err(Error {
                error_code: ResponseCode::INVALID_REQUEST.into(),
                message: format!("User flags {} is not a valid value", user.flags),
                ..Default::default()
            });
        }

        let mut users = self.users.lock().unwrap();
        if users.contains_key(&user.id) {
            println!("Invalid create user request, ID {} is already in use", user.id);
            return Err(Error {
                error_code: ResponseCode::ALREADY_EXISTS.into(),
                message: format!("User ID '{}' is already in use", user.id),
                ..Default::default()
            });
        }
        users.insert(user.id, Arc::new(User::new(user.id, user.flags, vehicle_configuration)));

        Ok(())
    }

    pub fn delete_user(&self, user_id: i32) -> Result<(), Error> {
        let mut users = self.users.lock().unwrap();

        if !users.contains_key(&user_id) {
            return Err(Error {
                error_code: ResponseCode::NOT_FOUND_ERROR.into(),
                message: format!("User {} does not exist", user_id),
                ..Default::default()
            });
        }

        users.remove(&user_id);
        println!("Deleted user {}", user_id);

        Ok(())
    }

    pub fn delete_all(&self) {
        let mut users = self.users.lock().unwrap();

        users.clear();
    }

    pub fn list_users(&self) -> Vec<UserProto> {
        let users = self.users.lock().unwrap();

        users
            .values()
            .map(|user| UserProto { id: user.id, flags: user.flags, ..Default::default() })
            .collect()
    }

    pub fn get_user(&self, user_id: i32) -> Option<Arc<User>> {
        let users = self.users.lock().unwrap();

        users.get(&user_id).cloned()
    }

    fn are_valid_user_flags(user_flags: i32) -> bool {
        // Ensure driver flag is set since only driver is supported now
        (user_flags & UserFlags::DRIVER.value()) == UserFlags::DRIVER.value() &&
        // Ensure that no other bits are set other than ephemeral & driver
        user_flags <=  (UserFlags::EPHEMERAL.value() | UserFlags::DRIVER.value())
    }
}
