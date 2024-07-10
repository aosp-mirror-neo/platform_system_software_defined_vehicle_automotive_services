// Copyright 2023 Google LLC
use itertools::Itertools;

use sdvgenerated::sdv_user_preferences_user_preferences_admin_service_interface::UserPreferencesAdminService;
use sdvgenerated::sdv_user_preferences_user_preferences_management_service_interface::UserPreferencesManagementService;
use sdvgenerated::sdv_user_preferences_user_preferences_registry_service_interface::UserPreferencesRegistryService;

use user_preferences_api::error::{Error, ResponseCode};
use user_preferences_api::setting::SettingDefinition as SettingDefinitionProto;
use user_preferences_api::user_preferences_admin_service::*;
use user_preferences_api::user_preferences_management_service::*;
use user_preferences_api::user_preferences_registry_service::{
    RegisterSettingsRequest, RegisterSettingsResponse, UpdateSettingsRequest,
    UpdateSettingsResponse,
};

use crate::macros::error_response;
use crate::persistence::setting_definition::SettingDefinition;
use crate::persistence::settings_definition_repository::*;
use crate::persistence::settings_group_repository::*;
use crate::persistence::user_repository::{User, UserRepository};
use log::{error, info};

use protobuf::{EnumOrUnknown, MessageField};
use sdvmiddleware_rpc::RpcResult;

pub struct UserPreferencesServiceImpl {
    user_repository: UserRepository,
    setting_group_repository: SettingsGroupRepository,
    setting_definition_repository: SettingDefinitionRepository,
}

impl UserPreferencesServiceImpl {
    pub fn new(
        user_repository: UserRepository,
        setting_group_repository: SettingsGroupRepository,
        setting_definition_repository: SettingDefinitionRepository,
    ) -> Self {
        UserPreferencesServiceImpl {
            user_repository,
            setting_group_repository,
            setting_definition_repository,
        }
    }
}

impl UserPreferencesAdminService for UserPreferencesServiceImpl {
    fn create_user(&self, request: CreateUserRequest) -> RpcResult<CreateUserResponse> {
        let user: User = match request.user.into_option() {
            Some(x) => x.into(),
            None => {
                return error_response!(
                    CreateUserResponse,
                    ResponseCode::INVALID_REQUEST,
                    "User cannot be empty",
                )
            }
        };
        info!("Creating user: {:?}", user);

        if let Err(err) = self.user_repository.add_user(&user) {
            match err {
                crate::persistence::error::Error::AlreadyExists => {
                    info!("User {} already exists", user.id);
                    return error_response!(
                        CreateUserResponse,
                        ResponseCode::INVALID_REQUEST,
                        "User {} already exists",
                        user.id
                    );
                }
                _ => {
                    error!("Unable to create user: {:?}", err);
                    return error_response!(
                        CreateUserResponse,
                        ResponseCode::INTERNAL_ERROR,
                        "Unable to add user",
                    );
                }
            }
        }

        info!("Successfully added user: {:?}", user);
        Ok(CreateUserResponse::new())
    }

    fn select_user(&self, _request: SelectUserRequest) -> RpcResult<SelectUserResponse> {
        RpcResult::Err(sdvmiddleware_rpc::error::RpcError::Other)
    }

    fn delete_user(&self, _request: DeleteUserRequest) -> RpcResult<DeleteUserResponse> {
        RpcResult::Err(sdvmiddleware_rpc::error::RpcError::Other)
    }

    fn factory_reset(&self, _request: FactoryResetRequest) -> RpcResult<FactoryResetResponse> {
        RpcResult::Err(sdvmiddleware_rpc::error::RpcError::Other)
    }

    fn list_users(&self, _request: ListUsersRequest) -> RpcResult<ListUsersResponse> {
        RpcResult::Err(sdvmiddleware_rpc::error::RpcError::Other)
    }

    fn get_user_settings(
        &self,
        _request: GetUserSettingsRequest,
    ) -> RpcResult<GetUserSettingsResponse> {
        RpcResult::Err(sdvmiddleware_rpc::error::RpcError::Other)
    }
}

impl UserPreferencesManagementService for UserPreferencesServiceImpl {
    fn request_settings_change(
        &self,
        _request: RequestSettingsChangeRequest,
    ) -> RpcResult<RequestSettingsChangeResponse> {
        RpcResult::Err(sdvmiddleware_rpc::error::RpcError::Other)
    }

    fn subscribe_to_settings_change_and_get_settings(
        &self,
        _request: SubscribeToSettingsChangeAndGetSettingsRequest,
    ) -> RpcResult<SubscribeToSettingsChangeAndGetSettingsResponse> {
        RpcResult::Err(sdvmiddleware_rpc::error::RpcError::Other)
    }

    fn unsubscribe_from_settings_change(
        &self,
        _request: UnsubscribeToSettingsChangeRequest,
    ) -> RpcResult<UnsubscribeToSettingsChangeResponse> {
        RpcResult::Err(sdvmiddleware_rpc::error::RpcError::Other)
    }
}

impl UserPreferencesRegistryService for UserPreferencesServiceImpl {
    fn register_settings(
        &self,
        request: RegisterSettingsRequest,
    ) -> RpcResult<RegisterSettingsResponse> {
        let group_id = match self.setting_group_repository.store(&SettingsGroup {
            id: 0,
            name: request.group_name,
            service_fqin: request.caller_fqin,
        }) {
            Ok(x) => x,
            Err(err) => {
                println!("Failed to register settings group: {:?}", err);
                return error_response!(
                    RegisterSettingsResponse,
                    ResponseCode::INTERNAL_ERROR,
                    "Unexpected error, unable to register settings",
                );
            }
        };

        let definition = request
            .settings_definitions
            .into_iter()
            .map(|definition_proto: SettingDefinitionProto| {
                let setting_and_constraints = definition_proto.setting_and_constraints.unwrap();
                let setting = setting_and_constraints.setting.unwrap();
                SettingDefinition {
                    name: setting.key,
                    kind: definition_proto.kind.value(),
                    default_value: setting.value.unwrap().into(),
                    constraints: setting_and_constraints.constraints.map(|x| x.into()),
                }
            })
            .collect_vec();

        match self.setting_definition_repository.store_all(group_id, &definition) {
            Ok(_) => Ok(RegisterSettingsResponse::new()),
            Err(err) => {
                println!("Failed to register setting definitions: {}", err);
                error_response!(
                    RegisterSettingsResponse,
                    ResponseCode::INTERNAL_ERROR,
                    "Unexpected error, unable to register settings",
                )
            }
        }
    }

    fn update_settings(
        &self,
        _request: UpdateSettingsRequest,
    ) -> RpcResult<UpdateSettingsResponse> {
        RpcResult::Err(sdvmiddleware_rpc::error::RpcError::Other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::sync::{Arc, Mutex};
    use user_preferences_api::setting::{
        setting::Value as ValueProto, setting_and_constraints::Constraints as ConstraintsProto,
        Int32Constraints as Int32ConstraintsProto, Setting as SettingProto,
        SettingAndConstraints as SettingAndConstraintsProto, SettingKind as SettingKindProto,
    };
    use user_preferences_api::user::User as UserProto;

    #[test]
    fn create_user_succeeds() {
        let user_preferences_impl = create_user_preferences_impl();

        let user_proto = UserProto { id: 1, flags: 0, ..Default::default() };
        let request =
            CreateUserRequest { user: MessageField::some(user_proto), ..Default::default() };

        let response = user_preferences_impl.create_user(request).unwrap();

        assert!(response.error.is_none());
    }

    #[test]
    fn create_user_fails_when_user_already_exists() {
        let user_preferences_impl = create_user_preferences_impl();

        let user_proto = UserProto { id: 1, flags: 0, ..Default::default() };
        let request = CreateUserRequest {
            user: MessageField::some(user_proto.clone()),
            ..Default::default()
        };

        // Create the user first
        user_preferences_impl.create_user(request).unwrap();

        // Attempt to create the same user again
        let response = user_preferences_impl
            .create_user(CreateUserRequest {
                user: MessageField::some(user_proto),
                ..Default::default()
            })
            .unwrap();

        // Assert that the response contains an error
        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().message, "User 1 already exists");
    }

    #[test]
    fn create_user_fails_for_empty_request() {
        let user_preferences_impl = create_user_preferences_impl();

        let request = CreateUserRequest { user: MessageField::none(), ..Default::default() };

        let response = user_preferences_impl.create_user(request).unwrap();

        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().message, "User cannot be empty");
    }

    #[test]
    fn register_settings_succeeds_when_called_multiple_times() {
        let user_preferences_impl = create_user_preferences_impl();

        let settings_definitions = vec![
            SettingDefinitionProto {
                kind: SettingKindProto::PER_USER.into(),
                setting_and_constraints: Some(SettingAndConstraintsProto {
                    setting: Some(SettingProto {
                        key: "bool_setting".to_string(),
                        value: Some(ValueProto::Bool(true)),
                        ..Default::default()
                    })
                    .into(),
                    constraints: None,
                    ..Default::default()
                })
                .into(),
                ..Default::default()
            },
            SettingDefinitionProto {
                kind: SettingKindProto::PER_USER.into(),
                setting_and_constraints: Some(SettingAndConstraintsProto {
                    setting: Some(SettingProto {
                        key: "int32_setting".to_string(),
                        value: Some(ValueProto::Int32(10)),
                        ..Default::default()
                    })
                    .into(),
                    constraints: Some(ConstraintsProto::Int32Constraints(Int32ConstraintsProto {
                        min_value: Some(0),
                        max_value: Some(20),
                        step: Some(1),
                        ..Default::default()
                    })),
                    ..Default::default()
                })
                .into(),
                ..Default::default()
            },
        ];

        let request = RegisterSettingsRequest {
            group_name: "test_group".to_string(),
            caller_fqin: "com.example.test_service".to_string(),
            settings_definitions: settings_definitions.clone(),
            ..Default::default()
        };

        // Register the settings group first
        user_preferences_impl.register_settings(request.clone()).unwrap();

        // Attempt to register the same settings group again
        let response = user_preferences_impl.register_settings(request).unwrap();

        // Assert that the response is ok
        assert!(response.error.is_none());
    }

    fn create_user_preferences_impl() -> UserPreferencesServiceImpl {
        let connection = Arc::new(Mutex::new(
            Connection::open_in_memory().expect("Failed to establish connection"),
        ));

        let user_repository = UserRepository::new(connection.clone())
            .expect("Failed to create an instance of user repository");

        let settings_group_repository = SettingsGroupRepository::new(connection.clone())
            .expect("Failed to create an instance of settings group repository");

        let settings_definition_repository = SettingDefinitionRepository::new(connection.clone())
            .expect("Failed to create an instance of settings definition repository");

        UserPreferencesServiceImpl::new(
            user_repository,
            settings_group_repository,
            settings_definition_repository,
        )
    }
}
