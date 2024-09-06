// Copyright 2024 Google LLC

use lazy_static::lazy_static;
use protobuf::MessageField;
use user_preferences_api::{
    setting::{
        setting::Value, setting_and_constraints::Constraints, Int32Constraints, Setting,
        SettingAndConstraints, SettingDefinition, SettingKind,
    },
    settings_group::{SettingsGroup, SettingsGroupId},
    user::User,
    user_preferences_admin_service::*,
    user_preferences_management_service::*,
    user_preferences_registry_service::*,
};

use crate::user_preferences_service_impl::UserPreferencesServiceImpl;
use sdvgenerated::sdv_user_preferences_user_preferences_admin_service_interface::UserPreferencesAdminService;
use sdvgenerated::sdv_user_preferences_user_preferences_management_service_interface::UserPreferencesManagementService;
use sdvgenerated::sdv_user_preferences_user_preferences_registry_service_interface::UserPreferencesRegistryService;

pub static TEST_SETTINGS_GROUP_NAME: &str = "test_group";
pub static TEST_USER_CONTROLLABLE_SERVICE_FQIN: &str = "com.example.test_user_controllable_service";
pub static TEST_SUBSCRIBER_FQIN: &str = "com.example.test_subscriber";

lazy_static! {
    pub static ref DEFAULT_TEST_SETTING_GROUP_ID: SettingsGroupId = SettingsGroupId {
        name: TEST_SETTINGS_GROUP_NAME.to_string(),
        service_fqin: TEST_USER_CONTROLLABLE_SERVICE_FQIN.to_string(),
        ..Default::default()
    };
    pub static ref DEFAULT_TEST_SETTING_GROUP: SettingsGroup = SettingsGroup {
        id: Some(DEFAULT_TEST_SETTING_GROUP_ID.clone()).into(),
        version: "1.0".to_string(),
        settings: vec![default_bool_setting(), default_int32_setting()],
        ..Default::default()
    };
}

pub fn create_user_and_assert_success(
    service_impl: &UserPreferencesServiceImpl,
    id: i32,
    flags: i32,
) {
    let user_proto = User { id, flags, ..Default::default() };
    let create_user_request =
        CreateUserRequest { user: MessageField::some(user_proto), ..Default::default() };
    let response = service_impl.create_user(create_user_request).unwrap();

    assert_eq!(
        response.error.clone().into_option(),
        None,
        "Expected create_user to succeed: {:?}",
        response.error
    );
}

pub fn register_default_settings_and_assert_success(service_impl: &UserPreferencesServiceImpl) {
    let request = RegisterSettingsRequest {
        group_name: TEST_SETTINGS_GROUP_NAME.to_string(),
        caller_fqin: TEST_USER_CONTROLLABLE_SERVICE_FQIN.to_string(),
        settings_definitions: vec![
            default_bool_setting_definition(),
            default_int32_setting_definition(),
        ],
        ..Default::default()
    };
    let response = service_impl.register_settings(request).unwrap();
    assert_eq!(
        response.error.clone().into_option(),
        None,
        "Expected register_settings to succeed: {:?}",
        response.error
    );
}

pub fn select_user_and_assert_success(service_impl: &UserPreferencesServiceImpl, user_id: i32) {
    let request = SelectUserRequest { user_id, ..Default::default() };
    let response = service_impl.select_user(request).unwrap();
    assert_eq!(
        response.error.clone().into_option(),
        None,
        "Expected select_user to succeed: {:?}",
        response.error
    );
}

pub fn subscribe_to_settings_change_and_get_settings(
    service_impl: &UserPreferencesServiceImpl,
    group_id: SettingsGroupId,
) -> SubscribeToSettingsChangeAndGetSettingsResponse {
    let request = SubscribeToSettingsChangeAndGetSettingsRequest {
        settings_group_id: Some(group_id).into(),
        caller_fqin: TEST_SUBSCRIBER_FQIN.to_string(),
        ..Default::default()
    };
    service_impl.subscribe_to_settings_change_and_get_settings(request).unwrap()
}

pub fn default_bool_setting() -> Setting {
    let setting_and_constraint = default_bool_setting_definition().setting_and_constraints.unwrap();
    setting_and_constraint.setting.unwrap()
}

pub fn default_bool_setting_definition() -> SettingDefinition {
    SettingDefinition {
        kind: SettingKind::PER_USER.into(),
        setting_and_constraints: Some(SettingAndConstraints {
            setting: Some(Setting {
                key: "bool_setting".to_string(),
                value: Some(Value::Bool(true)),
                ..Default::default()
            })
            .into(),
            constraints: None,
            ..Default::default()
        })
        .into(),
        ..Default::default()
    }
}

pub fn default_int32_setting() -> Setting {
    let setting_and_constraint =
        default_int32_setting_definition().setting_and_constraints.unwrap();
    setting_and_constraint.setting.unwrap()
}

pub fn default_int32_setting_definition() -> SettingDefinition {
    SettingDefinition {
        kind: SettingKind::PER_USER.into(),
        setting_and_constraints: Some(SettingAndConstraints {
            setting: Some(Setting {
                key: "int32_setting".to_string(),
                value: Some(Value::Int32(10)),
                ..Default::default()
            })
            .into(),
            constraints: Some(Constraints::Int32Constraints(Int32Constraints {
                min_value: Some(0),
                max_value: Some(20),
                step: Some(1),
                ..Default::default()
            })),
            ..Default::default()
        })
        .into(),
        ..Default::default()
    }
}
