#[cfg(test)]
mod tests {
    use named_lock::NamedLock;
    use protobuf::{Enum, MessageField};
    use sdvgenerated::mock_user_preferences_client::MockUserPreferencesClient;
    use sdvgenerated::sdv_user_preferences_user_preferences_admin_service_interface::UserPreferencesAdminService;
    use sdvgenerated::sdv_user_preferences_user_preferences_registry_service_interface::UserPreferencesRegistryService;
    use sdvmiddleware_rpc::RpcResult;
    use std::process::Command;
    use user_preferences_api::error::ResponseCode;
    use user_preferences_api::setting::setting::Value;
    use user_preferences_api::setting::setting_and_constraints::Constraints;
    use user_preferences_api::setting::Int64Constraints;
    use user_preferences_api::setting::{
        Setting, SettingAndConstraints, SettingDefinition, SettingKind,
    };
    use user_preferences_api::user::{user::UserFlags, User};
    use user_preferences_api::user_preferences_admin_service::CreateUserRequest;
    use user_preferences_api::user_preferences_registry_service::{
        RegisterSettingsRequest, RegisterSettingsResponse, UpdateSettingsRequest,
        UpdateSettingsResponse,
    };
    const USER_PREFERENCES_FQIN: &str = "UserPreferencesServer";
    const MOCK_USER_CONTROLLABLE_SERVICE_FQIN: &str = "MockUserControllableService";
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

    #[test]
    fn user_controllable_service_can_register_settings() {
        let lock = NamedLock::create("test_lock").unwrap();
        let _guard = lock.lock().unwrap();

        let test_setup: TestSetup = TestSetup::new();

        let setting_definitions = vec![create_setting_definition(
            "mock_setting",
            Value::Int64(1),
            SettingKind::PER_USER,
            Some(i64_setting_constraints(1, 5)),
        )];

        let response = register_settings(&test_setup, "MockSettingsGroup", setting_definitions)
            .expect("Failed to perform request to register setting response");
        assert!(response.error.is_none(), "Expected setting registration to succeed");
    }

    #[test]
    fn user_controllable_service_can_update_settings() {
        let lock = NamedLock::create("test_lock").unwrap();
        let _guard = lock.lock().unwrap();

        let test_setup: TestSetup = TestSetup::new();

        // register settings so we can update them
        const MOCK_SETTING_NAME: &str = "mock_setting";

        let setting_definitions = vec![create_setting_definition(
            MOCK_SETTING_NAME,
            Value::Int64(1),
            SettingKind::PER_USER,
            Some(i64_setting_constraints(1, 5)),
        )];
        const MOCK_SETTING_GROUP_NAME: &str = "MockSettingsGroup";

        let response = register_settings(&test_setup, MOCK_SETTING_GROUP_NAME, setting_definitions)
            .expect("Failed to perform request to register setting response");
        assert!(response.error.is_none(), "Expected setting registration to succeed");

        let settings = vec![Setting {
            key: MOCK_SETTING_NAME.to_string(),
            value: Some(Value::Int64(2)),
            ..Default::default()
        }];
        let response = update_settings(&test_setup, MOCK_SETTING_GROUP_NAME, settings)
            .expect("Failed to perform request to update setting");
        assert!(
            response.error.is_none(),
            "Expected setting update to succeed: {:?}",
            response.error
        );
    }

    fn register_settings(
        test_setup: &TestSetup,
        setting_group_name: &str,
        settings_definitions: Vec<SettingDefinition>,
    ) -> RpcResult<RegisterSettingsResponse> {
        let client = test_setup
            .mock_user_preferences_clients
            .get_sdv_user_preferences_user_preferences_registry_service_client(
                USER_PREFERENCES_FQIN,
            );
        client.register_settings(RegisterSettingsRequest {
            group_name: setting_group_name.to_string(),
            version: String::from("1.0"),
            settings_definitions,
            caller_fqin: MOCK_USER_CONTROLLABLE_SERVICE_FQIN.to_string(),
            ..Default::default()
        })
    }

    fn update_settings(
        test_setup: &TestSetup,
        setting_group_name: &str,
        settings: Vec<Setting>,
    ) -> RpcResult<UpdateSettingsResponse> {
        let client = test_setup
            .mock_user_preferences_clients
            .get_sdv_user_preferences_user_preferences_registry_service_client(
                USER_PREFERENCES_FQIN,
            );

        client.update_settings(UpdateSettingsRequest {
            group_name: setting_group_name.to_string(),
            settings,
            caller_fqin: MOCK_USER_CONTROLLABLE_SERVICE_FQIN.to_string(),
            ..Default::default()
        })
    }

    fn create_setting_definition(
        setting_name: &str,
        value: Value,
        setting_kind: SettingKind,
        constraints: Option<Constraints>,
    ) -> SettingDefinition {
        SettingDefinition {
            kind: setting_kind.into(),
            setting_and_constraints: MessageField::some(SettingAndConstraints {
                setting: MessageField::some(Setting {
                    key: setting_name.to_string(),
                    value: Some(value),
                    ..Default::default()
                }),
                constraints,
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn i64_setting_constraints(min: i64, max: i64) -> Constraints {
        Constraints::Int64Constraints(Int64Constraints {
            min_value: Some(min),
            max_value: Some(max),
            ..Default::default()
        })
    }
}
