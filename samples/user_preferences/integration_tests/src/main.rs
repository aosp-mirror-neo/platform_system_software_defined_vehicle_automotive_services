#[cfg(test)]
mod tests {
    use named_lock::NamedLock;
    use protobuf::{Enum, MessageField};
    use sdvgenerated::mock_user_preferences_client::MockUserPreferencesClient;
    use sdvgenerated::sdv_user_preferences_user_preferences_admin_service_interface::UserPreferencesAdminService;
    use std::process::Command;
    use user_preferences_api::error::ResponseCode;
    use user_preferences_api::user::{user::UserFlags, User};
    use user_preferences_api::user_preferences_admin_service::CreateUserRequest;
    const USER_PREFERENCES_SERVICE_BINARY: &str = "/vendor/bin/sdv_user_preferences_service";

    struct TestSetup {
        pub user_preferences_service: std::process::Child,
        pub mock_user_preferences_clients: MockUserPreferencesClient,
    }
    impl TestSetup {
        pub fn new() -> Self {
            log::info!("Setting up test environment");
            let test_setup = Self {
                user_preferences_service: Command::new(USER_PREFERENCES_SERVICE_BINARY)
                    .spawn()
                    .expect("Failed to launch User Preferences binary"),

                mock_user_preferences_clients: MockUserPreferencesClient::new(),
            };

            // add a sleep to ensure tests cannot start before server starts
            // TODO:(b/339145709) Replace this sleep with wait for service
            // API that will be provided by middleware once available
            std::thread::sleep(std::time::Duration::from_millis(100));

            test_setup
        }
    }

    impl Drop for TestSetup {
        fn drop(&mut self) {
            match self.user_preferences_service.try_wait() {
                Ok(Some(status)) => log::info!("exited with: {status}"),
                Ok(None) => {
                    self.user_preferences_service.kill().expect("command couldn't be killed");
                }
                Err(e) => log::info!("error` attempting to wait: {e}"),
            }
        }
    }

    #[test]
    fn create_user_can_succeed() {
        let lock = NamedLock::create("test_lock").unwrap();
        let _guard = lock.lock().unwrap();

        //TODO(b/339148501) use a global instance the user preferences and mock user preferences client
        //instead of creating an instance within each test.
        let test_setup: TestSetup = TestSetup::new();
        let client = test_setup
            .mock_user_preferences_clients
            .get_sdv_user_preferences_user_preferences_admin_service_client(
                "UserPreferencesServer",
            );

        let response = client
            .create_user(CreateUserRequest {
                user: MessageField::some(User {
                    id: 1_i32,
                    flags: UserFlags::DRIVER.value() | UserFlags::EPHEMERAL.value(),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .expect("Failed to perform request to create user");

        assert_eq!(response.error.into_option(), None, "Create User returned a unexpected error");
    }

    #[test]
    fn create_user_returns_error_when_user_already_exists() {
        let lock = NamedLock::create("test_lock").unwrap();
        let _guard = lock.lock().unwrap();

        let test_setup: TestSetup = TestSetup::new();
        let client = test_setup
            .mock_user_preferences_clients
            .get_sdv_user_preferences_user_preferences_admin_service_client(
                "UserPreferencesServer",
            );

        for i in 0..2 {
            let response = client
                .create_user(CreateUserRequest {
                    user: MessageField::some(User {
                        id: 2_i32,
                        flags: UserFlags::DRIVER.value() | UserFlags::EPHEMERAL.value(),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .expect("Failed to perform request to create user");

            log::info!("finished user create request");
            if i == 0 {
                assert_eq!(
                    response.error.into_option(),
                    None,
                    "Create User returned a unexpected error"
                )
            } else {
                assert!(response.error.is_some(), "Expected create user to return an error");
                assert!(
                    response.error.error_code == ResponseCode::ALREADY_EXISTS.into(),
                    "Expected error to have status code already exists"
                )
            }
        }
    }

    #[test]
    fn create_user_returns_error_when_user_field_is_none() {
        let lock = NamedLock::create("test_lock").unwrap();
        let _guard = lock.lock().unwrap();

        let test_setup: TestSetup = TestSetup::new();
        let client = test_setup
            .mock_user_preferences_clients
            .get_sdv_user_preferences_user_preferences_admin_service_client(
                "UserPreferencesServer",
            );

        let response = client
            .create_user(CreateUserRequest { ..Default::default() })
            .expect("Failed to perform request to create user");

        assert!(response.error.is_some(), "Expected create user to return an error");
        assert!(
            response.error.error_code == ResponseCode::INVALID_REQUEST.into(),
            "Expected error to have status code invalid_request"
        )
    }

    #[test]
    fn create_user_returns_error_when_user_flags_are_incorrect() {
        let lock = NamedLock::create("test_lock").unwrap();
        let _guard = lock.lock().unwrap();

        let test_setup: TestSetup = TestSetup::new();
        let client = test_setup
            .mock_user_preferences_clients
            .get_sdv_user_preferences_user_preferences_admin_service_client(
                "UserPreferencesServer",
            );

        let response = client
            .create_user(CreateUserRequest {
                user: MessageField::some(User { id: 3_i32, ..Default::default() }),
                ..Default::default()
            })
            .expect("Failed to perform request to create user");

        assert!(
            response.error.error_code == ResponseCode::INVALID_REQUEST.into(),
            "Expected error to have status code invalid_request"
        )
    }
}
