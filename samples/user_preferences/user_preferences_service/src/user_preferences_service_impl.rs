// Copyright 2023 Google LLC
use anyhow::Result;
use itertools::Itertools;
use log::{debug, error, info, warn};

use sdvgenerated::sdv_user_preferences_user_controllable_user_controllable_service_interface::UserControllableService;
use sdvgenerated::sdv_user_preferences_user_preferences_admin_service_interface::UserPreferencesAdminService;
use sdvgenerated::sdv_user_preferences_user_preferences_management_service_interface::UserPreferencesManagementService;
use sdvgenerated::sdv_user_preferences_user_preferences_registry_service_interface::UserPreferencesRegistryService;
use user_preferences_api::user_preferences_management_service::request_settings_change_request::ChangePersistencePolicy;

use user_preferences_api::error::{Error, ResponseCode};
use user_preferences_api::setting::Setting as SettingProto;
use user_preferences_api::setting::{SettingDefinition as SettingDefinitionProto, SettingKind};
use user_preferences_api::user_controllable_service;
use user_preferences_api::user_preferences_admin_service::*;
use user_preferences_api::user_preferences_management_service::*;
use user_preferences_api::user_preferences_registry_service::{
    RegisterSettingsRequest, RegisterSettingsResponse, UpdateSettingsRequest,
    UpdateSettingsResponse,
};

use crate::macros::error_response;
use crate::persistence::setting::Setting;
use crate::persistence::setting_definition::SettingDefinition;
use crate::persistence::settings_definition_repository::*;
use crate::persistence::settings_group_repository::*;
use crate::persistence::user_repository::{User, UserRepository};
use crate::persistence::user_settings_repository::UserSettingsRepository;
use crate::persistence::vehicle_settings_repository::VehicleSettingsRepository;
use crate::vsidl_client_provider::VSIDLClientProvider;

use protobuf::{EnumOrUnknown, MessageField};
use sdvmiddleware_rpc::RpcResult;
use std::collections::HashMap;
use std::sync::{Mutex, Weak};

pub struct UserPreferencesServiceImplArgs {
    pub user_repository: UserRepository,
    pub settings_group_repository: SettingsGroupRepository,
    pub settings_definition_repository: SettingDefinitionRepository,
    pub user_settings_repository: UserSettingsRepository,
    pub vehicle_settings_repository: VehicleSettingsRepository,
    pub vsidl_client_provider: Weak<dyn VSIDLClientProvider>,
}

pub struct UserPreferencesServiceImpl {
    user_repository: UserRepository,
    settings_group_repository: SettingsGroupRepository,
    settings_definition_repository: SettingDefinitionRepository,
    user_settings_repository: UserSettingsRepository,
    vehicle_settings_repository: VehicleSettingsRepository,
    vsidl_client_provider: Weak<dyn VSIDLClientProvider>,
    active_user_id: Mutex<Option<i32>>,
}

impl UserPreferencesServiceImpl {
    pub fn new(args: UserPreferencesServiceImplArgs) -> Self {
        UserPreferencesServiceImpl {
            user_repository: args.user_repository,
            settings_group_repository: args.settings_group_repository,
            settings_definition_repository: args.settings_definition_repository,
            user_settings_repository: args.user_settings_repository,
            vehicle_settings_repository: args.vehicle_settings_repository,
            vsidl_client_provider: args.vsidl_client_provider,
            active_user_id: Mutex::new(None),
        }
    }

    pub fn calculate_group_settings_for_user(
        &self,
        user_id: i32,
        settings_group_id: i64,
    ) -> Result<Vec<Setting>> {
        // Since this method only determines the settings for the specific user,
        // Settings of Kind SHARED or PASSTHROUGH should be filtered out
        let default_settings = self
            .settings_definition_repository
            .get(settings_group_id)?
            .into_iter()
            .filter(|s| s.kind == SettingKind::PER_USER as i32)
            .collect_vec();

        let user_overrides = self.user_settings_repository.get_all(user_id, settings_group_id)?;

        let effective_settings = self.combine_settings(default_settings, user_overrides);

        Ok(effective_settings)
    }

    fn combine_settings(
        &self,
        defaults: Vec<SettingDefinition>,
        overrides: Vec<Setting>,
    ) -> Vec<Setting> {
        let mut effective_settings = HashMap::new();

        for default_setting in defaults.into_iter() {
            effective_settings.insert(default_setting.name.clone(), Setting::from(default_setting));
        }

        for r#override in overrides.into_iter() {
            effective_settings.insert(r#override.name.clone(), r#override);
        }

        effective_settings.into_values().collect()
    }

    fn are_valid_settings(
        &self,
        settings_group: &SettingsGroup,
        settings: &[SettingProto],
    ) -> Result<(), Error> {
        let settings_definitions: Vec<SettingDefinition> =
            self.settings_definition_repository.get(settings_group.id).map_err(|err| Error {
                error_code: ResponseCode::INTERNAL_ERROR.into(),
                message: format!(
                    "Unable to retrieve settings definitions for validation: {:?}",
                    err
                ),
                ..Default::default()
            })?;
        let settings_definitions: HashMap<String, SettingDefinition> = settings_definitions
            .into_iter()
            .map(|setting_definition| (setting_definition.name.clone(), setting_definition))
            .collect::<HashMap<_, _>>();

        for setting in settings {
            let setting_definition =
                settings_definitions.get(&setting.key).ok_or_else(|| Error {
                    error_code: ResponseCode::INVALID_REQUEST.into(),
                    message: format!("Setting {} not found in settings group", setting.key),
                    ..Default::default()
                })?;

            setting_definition.is_valid_value(&setting.value.clone().unwrap().into()).map_err(
                |err| Error {
                    error_code: ResponseCode::INVALID_REQUEST.into(),
                    message: format!("{}", err),
                    ..Default::default()
                },
            )?;
        }

        Ok(())
    }

    fn apply_settings(
        &self,
        settings_group: SettingsGroup,
        settings: Vec<SettingProto>,
    ) -> Result<(), String> {
        let client = match self.vsidl_client_provider.upgrade() {
            Some(x) => x
                .get_sdv_user_preferences_user_controllable_user_controllable_service_client(
                    &settings_group.service_fqin,
                ),
            None => return Err(String::from("VSIDL client provider is not available")),
        };

        let resp = client
            .request_settings_change(user_controllable_service::RequestSettingsChangeRequest {
                group_name: settings_group.name.clone(),
                settings,
                ..Default::default()
            })
            .map_err(|err| {
                format!("Failed to perform GRPC request to request_settings_change: {}", err)
            })?;

        if let Some(err) = resp.error.into_option() {
            return Err(format!(
                "User Controllable Service failed to apply setting change: {}",
                err
            ));
        }

        match self
            .vehicle_settings_repository
            .store_all(settings_group.id, resp.settings.into_iter().map_into().collect_vec())
        {
            Ok(_) => Ok(()),
            Err(err) => Err(format!("Failed to update internal vehicle state table {}", err)),
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
                    warn!("User {} already exists", user.id);
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

    fn select_user(&self, request: SelectUserRequest) -> RpcResult<SelectUserResponse> {
        let user = match self.user_repository.get_user(request.user_id) {
            Ok(x) => x,
            Err(err) => match err {
                crate::persistence::error::Error::NotFound => {
                    warn!("User {} doesn't exist", request.user_id);
                    return error_response!(
                        SelectUserResponse,
                        ResponseCode::NOT_FOUND_ERROR,
                        "User {} does not exist",
                        request.user_id
                    );
                }
                _ => {
                    error!("Unable to switch to user: {:?}", err);
                    return error_response!(
                        SelectUserResponse,
                        ResponseCode::INTERNAL_ERROR,
                        "Unable to switch to user: {:?}",
                        err
                    );
                }
            },
        };

        *self.active_user_id.lock().unwrap() = Some(user.id);

        info!("Switching to user {}", user.id);

        let groups = match self.settings_group_repository.list() {
            Ok(x) => x,
            Err(err) => {
                error!("Failed to list registered settings group: {}", err);
                return error_response!(
                    SelectUserResponse,
                    ResponseCode::INTERNAL_ERROR,
                    "Encountered unexpected error",
                );
            }
        };

        for group in groups.iter() {
            let settings = self.calculate_group_settings_for_user(user.id, group.id).unwrap();

            debug!(
                "Loaded settings for group {:?}\n Applying the following settings: {:?}",
                group, settings
            );

            if let Err(err) =
                self.apply_settings(group.clone(), settings.into_iter().map_into().collect())
            {
                error!(
                    "Failed to apply settings from settings group {}/{}: {}",
                    group.service_fqin, group.name, err
                );
            }
        }

        Ok(SelectUserResponse::new())
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
        request: RequestSettingsChangeRequest,
    ) -> RpcResult<RequestSettingsChangeResponse> {
        let settings_group_id = match request.settings_group_id.into_option() {
            Some(x) => x,
            None => {
                return error_response!(
                    RequestSettingsChangeResponse,
                    ResponseCode::INVALID_REQUEST,
                    "Settings group id cannot be empty",
                )
            }
        };
        let settings_group = match self
            .settings_group_repository
            .get(settings_group_id.name.as_str(), settings_group_id.service_fqin.as_str())
        {
            Ok(x) => x,
            Err(err) => match err {
                crate::persistence::error::Error::NotFound => {
                    info!(
                        "Settings group {}/{} doesn't exist",
                        settings_group_id.service_fqin, settings_group_id.name
                    );
                    return error_response!(
                        RequestSettingsChangeResponse,
                        ResponseCode::INVALID_REQUEST,
                        "Settings group {}/{} doesn't exist",
                        settings_group_id.service_fqin,
                        settings_group_id.name,
                    );
                }
                _ => {
                    error!("Failed to request setting change: {:?}", err);
                    return error_response!(
                        RequestSettingsChangeResponse,
                        ResponseCode::INTERNAL_ERROR,
                        "Unable to request setting change",
                    );
                }
            },
        };
        if let Err(err) = self.are_valid_settings(&settings_group, &request.settings) {
            warn!("Received request containing invalid settings: {:?}", err);
            return error_response!(
                RequestSettingsChangeResponse,
                ResponseCode::INVALID_REQUEST,
                "{}",
                err,
            );
        }
        if request.change_persistence_policy == ChangePersistencePolicy::PERSISTENT_CHANGE.into() {
            let current_user_id = match *self.active_user_id.lock().unwrap() {
                Some(x) => x,
                None => return error_response!(
                    RequestSettingsChangeResponse,
                    ResponseCode::INVALID_REQUEST,
                    "Change Persistence Policy cannot be `PERSISTENT_CHANGE` when no user is selected",
                ),
            };

            let settings: Vec<Setting> = request.settings.iter().cloned().map_into().collect();
            if let Err(err) = self.user_settings_repository.store_all(
                current_user_id,
                settings_group.id,
                &settings,
            ) {
                error!("Unable to update user settings table: {}", err);
                return error_response!(
                    RequestSettingsChangeResponse,
                    ResponseCode::INTERNAL_ERROR,
                    "Failed to request setting change",
                );
            }
        }

        //TODO: Implement asynchronous call to `apply_settings` once support for pending changes is added.
        //Pending changes can only be implemented after Service Discovery unit ty`pe APIs are implemented.
        if let Err(err) = self.apply_settings(settings_group.clone(), request.settings.clone()) {
            error!("Failed to apply requested settings: {}", err);
            //TODO: Once subscriptions are implemented & this call is done Asynchronosly, we need to callback
            //clients to notify them of current setting that is applied.
        }

        Ok(RequestSettingsChangeResponse::new())
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
        let group_id = match self.settings_group_repository.store(&SettingsGroup {
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
                    constraint: setting_and_constraints.constraints.map(|x| x.into()),
                }
            })
            .collect_vec();

        match self.settings_definition_repository.store(group_id, &definition) {
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
    use crate::vsidl_client_provider::{MockClientTransport, MockVSIDLClientProvider};
    use protobuf::Message;
    use rusqlite::Connection;
    use sdvgenerated_rpc_sdv_user_preferences_user_controllable_user_controllable_service::client::UserControllableServiceClient;
    use std::sync::{Arc, Mutex};
    use user_preferences_api::setting::{
        setting::Value as ValueProto, setting_and_constraints::Constraints as ConstraintsProto,
        Int32Constraints as Int32ConstraintsProto, Setting as SettingProto,
        SettingAndConstraints as SettingAndConstraintsProto, SettingKind as SettingKindProto,
    };
    use user_preferences_api::settings_group::SettingsGroupId as SettingsGroupIdProto;
    use user_preferences_api::user::User as UserProto;

    #[test]
    fn create_user_succeeds() {
        let mock_vsidl_client_provider: Arc<dyn VSIDLClientProvider> =
            Arc::new(MockVSIDLClientProvider::new());
        let user_preferences_impl =
            create_user_preferences_server_and_impl(Arc::downgrade(&mock_vsidl_client_provider));

        let user_proto = UserProto { id: 1, flags: 0, ..Default::default() };
        let request =
            CreateUserRequest { user: MessageField::some(user_proto), ..Default::default() };

        let response = user_preferences_impl.create_user(request).unwrap();

        assert!(response.error.is_none());
    }

    #[test]
    fn create_user_fails_when_user_already_exists() {
        let mock_vsidl_client_provider: Arc<dyn VSIDLClientProvider> =
            Arc::new(MockVSIDLClientProvider::new());
        let user_preferences_impl =
            create_user_preferences_server_and_impl(Arc::downgrade(&mock_vsidl_client_provider));

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
        let mock_vsidl_client_provider: Arc<dyn VSIDLClientProvider> =
            Arc::new(MockVSIDLClientProvider::new());
        let user_preferences_impl =
            create_user_preferences_server_and_impl(Arc::downgrade(&mock_vsidl_client_provider));

        let request = CreateUserRequest { user: MessageField::none(), ..Default::default() };

        let response = user_preferences_impl.create_user(request).unwrap();

        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().message, "User cannot be empty");
    }

    #[test]
    fn register_settings_succeeds_when_called_multiple_times() {
        let mock_vsidl_client_provider: Arc<dyn VSIDLClientProvider> =
            Arc::new(MockVSIDLClientProvider::new());
        let user_preferences_impl =
            create_user_preferences_server_and_impl(Arc::downgrade(&mock_vsidl_client_provider));

        let settings_definitions =
            vec![default_bool_setting_definition(), default_int32_setting_definition()];

        let request = RegisterSettingsRequest {
            group_name: "test_group".to_string(),
            caller_fqin: "com.example.test_service".to_string(),
            settings_definitions,
            ..Default::default()
        };

        // Register the settings group first
        user_preferences_impl.register_settings(request.clone()).unwrap();

        // Attempt to register the same settings group again
        let response = user_preferences_impl.register_settings(request).unwrap();

        // Assert that the response is ok
        assert!(response.error.is_none());
    }

    #[test]
    fn select_user_succeeds() {
        let mut mock_client_transport = Box::new(MockClientTransport::new());
        let mut mock_vsidl_client_provider = MockVSIDLClientProvider::new();

        let expected_rpc_request =
            user_preferences_api::user_controllable_service::RequestSettingsChangeRequest {
                group_name: String::from("test_group"),
                settings: vec![default_int32_setting(), default_bool_setting()],
                ..Default::default()
            };
        let rpc_response =
            user_preferences_api::user_controllable_service::RequestSettingsChangeResponse {
                settings: expected_rpc_request.settings.clone(),
                ..Default::default()
            };

        mock_client_transport
            .expect_call_method()
            .times(1)
            .withf(move |method, request| {
                let request =
                    user_controllable_service::RequestSettingsChangeRequest::parse_from_bytes(
                        request,
                    )
                    .unwrap();
                method == "UserControllableService::request_settings_change"
                    && request.group_name == expected_rpc_request.group_name
                    && are_settings_equal(&request.settings, &expected_rpc_request.settings)
            })
            .returning(move |_, _| Ok(rpc_response.write_to_bytes().unwrap()));

        let client = Arc::new(UserControllableServiceClient::new(mock_client_transport));
        // Set up expectations for the mock VSIDL client provider
        mock_vsidl_client_provider
            .expect_get_sdv_user_preferences_user_controllable_user_controllable_service_client()
            .times(1)
            .withf(|target_service_bundle| target_service_bundle == "com.example.test_service")
            .returning(move |_| client.clone());

        let mock_vsidl_client_provider: Arc<dyn VSIDLClientProvider> =
            Arc::new(mock_vsidl_client_provider);
        let user_preferences_impl =
            create_user_preferences_server_and_impl(Arc::downgrade(&mock_vsidl_client_provider));

        // Create a user
        let user_proto = UserProto { id: 1, flags: 0, ..Default::default() };
        let create_user_request =
            CreateUserRequest { user: MessageField::some(user_proto), ..Default::default() };
        user_preferences_impl.create_user(create_user_request).unwrap();

        // Register a settings group
        let settings_definitions =
            vec![default_bool_setting_definition(), default_int32_setting_definition()];
        let register_settings_request = RegisterSettingsRequest {
            group_name: "test_group".to_string(),
            caller_fqin: "com.example.test_service".to_string(),
            settings_definitions,
            ..Default::default()
        };
        user_preferences_impl.register_settings(register_settings_request).unwrap();

        // Select the user
        let select_user_request = SelectUserRequest { user_id: 1, ..Default::default() };
        let response = user_preferences_impl.select_user(select_user_request).unwrap();

        // Assert that the response is ok
        assert!(response.error.is_none());
    }

    #[test]
    fn select_user_fails_when_user_does_not_exist() {
        let mock_vsidl_client_provider: Arc<dyn VSIDLClientProvider> =
            Arc::new(MockVSIDLClientProvider::new());
        let user_preferences_impl =
            create_user_preferences_server_and_impl(Arc::downgrade(&mock_vsidl_client_provider));

        // Attempt to select a non-existent user
        let select_user_request = SelectUserRequest { user_id: 1, ..Default::default() };
        let response = user_preferences_impl.select_user(select_user_request).unwrap();

        // Assert that the response contains an error
        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().message, "User 1 does not exist");
    }

    #[test]
    fn request_settings_change_succeeds_for_valid_settings() {
        let mut mock_client_transport = Box::new(MockClientTransport::new());
        let mut mock_vsidl_client_provider = MockVSIDLClientProvider::new();

        let expected_rpc_request =
            user_preferences_api::user_controllable_service::RequestSettingsChangeRequest {
                group_name: String::from("test_group"),
                settings: vec![default_int32_setting(), default_bool_setting()],
                ..Default::default()
            };
        let rpc_response =
            user_preferences_api::user_controllable_service::RequestSettingsChangeResponse {
                settings: expected_rpc_request.settings.clone(),
                ..Default::default()
            };

        mock_client_transport
            .expect_call_method()
            .times(1)
            .withf(move |method, request| {
                let request =
                    user_controllable_service::RequestSettingsChangeRequest::parse_from_bytes(
                        request,
                    )
                    .unwrap();
                method == "UserControllableService::request_settings_change"
                    && request.group_name == expected_rpc_request.group_name
                    && are_settings_equal(&request.settings, &expected_rpc_request.settings)
            })
            .returning(move |_, _| Ok(rpc_response.write_to_bytes().unwrap()));

        let client = Arc::new(UserControllableServiceClient::new(mock_client_transport));
        // Set up expectations for the mock VSIDL client provider
        mock_vsidl_client_provider
            .expect_get_sdv_user_preferences_user_controllable_user_controllable_service_client()
            .times(1)
            .withf(|target_service_bundle| {
                info!(
                    "Are we returning a vsidl client provider: {} | {}",
                    target_service_bundle,
                    target_service_bundle == "com.example.test_service"
                );
                target_service_bundle == "com.example.test_service"
            })
            .returning(move |_| client.clone());

        let mock_vsidl_client_provider: Arc<dyn VSIDLClientProvider> =
            Arc::new(mock_vsidl_client_provider);
        let user_preferences_impl =
            create_user_preferences_server_and_impl(Arc::downgrade(&mock_vsidl_client_provider));

        // Register a settings group
        let settings_definitions =
            vec![default_bool_setting_definition(), default_int32_setting_definition()];
        let register_settings_request = RegisterSettingsRequest {
            group_name: "test_group".to_string(),
            caller_fqin: "com.example.test_service".to_string(),
            settings_definitions,
            ..Default::default()
        };
        let register_response = user_preferences_impl
            .register_settings(register_settings_request)
            .expect("Failed to register settings");
        assert_eq!(register_response.error.into_option(), None);

        // Request settings change with valid settings
        let request_settings_change_request = RequestSettingsChangeRequest {
            settings_group_id: Some(SettingsGroupIdProto {
                name: "test_group".to_string(),
                service_fqin: "com.example.test_service".to_string(),
                ..Default::default()
            })
            .into(),
            settings: vec![default_bool_setting(), default_int32_setting()],
            change_persistence_policy: ChangePersistencePolicy::NON_PERSISTENT_CHANGE.into(),
            ..Default::default()
        };
        let response =
            user_preferences_impl.request_settings_change(request_settings_change_request).unwrap();

        // Assert that the response is ok
        assert!(response.error.is_none());
    }

    #[test]
    fn request_settings_change_fails_for_invalid_settings() {
        let mock_vsidl_client_provider: Arc<dyn VSIDLClientProvider> =
            Arc::new(MockVSIDLClientProvider::new());
        let user_preferences_impl =
            create_user_preferences_server_and_impl(Arc::downgrade(&mock_vsidl_client_provider));

        // Register a settings group
        let settings_definitions =
            vec![default_bool_setting_definition(), default_int32_setting_definition()];
        let register_settings_request = RegisterSettingsRequest {
            group_name: "test_group".to_string(),
            caller_fqin: "com.example.test_service".to_string(),
            settings_definitions,
            ..Default::default()
        };
        user_preferences_impl.register_settings(register_settings_request).unwrap();

        // Request settings change with invalid settings
        let request_settings_change_request = RequestSettingsChangeRequest {
            settings_group_id: Some(SettingsGroupIdProto {
                name: "test_group".to_string(),
                service_fqin: "com.example.test_service".to_string(),
                ..Default::default()
            })
            .into(),
            settings: vec![SettingProto {
                key: "int32_setting".to_string(),
                value: Some(ValueProto::Int32(25)), // Invalid value
                ..Default::default()
            }],
            change_persistence_policy: ChangePersistencePolicy::NON_PERSISTENT_CHANGE.into(),
            ..Default::default()
        };
        let response =
            user_preferences_impl.request_settings_change(request_settings_change_request).unwrap();

        // Assert that the response contains an error
        assert!(response.error.is_some());
    }

    #[test]
    fn request_settings_change_fails_for_non_existent_settings_group() {
        let mock_vsidl_client_provider: Arc<dyn VSIDLClientProvider> =
            Arc::new(MockVSIDLClientProvider::new());
        let user_preferences_impl =
            create_user_preferences_server_and_impl(Arc::downgrade(&mock_vsidl_client_provider));

        // Create a user
        let user_proto = UserProto { id: 1, flags: 0, ..Default::default() };
        let create_user_request =
            CreateUserRequest { user: MessageField::some(user_proto), ..Default::default() };
        user_preferences_impl.create_user(create_user_request).unwrap();

        // Select the user
        let select_user_request = SelectUserRequest { user_id: 1, ..Default::default() };
        user_preferences_impl.select_user(select_user_request).unwrap();

        // Request settings change for a non-existent settings group
        let request_settings_change_request = RequestSettingsChangeRequest {
            settings_group_id: Some(SettingsGroupIdProto {
                name: "non_existent_group".to_string(),
                service_fqin: "com.example.test_service".to_string(),
                ..Default::default()
            })
            .into(),
            settings: vec![default_int32_setting()],
            change_persistence_policy: ChangePersistencePolicy::NON_PERSISTENT_CHANGE.into(),
            ..Default::default()
        };
        let response =
            user_preferences_impl.request_settings_change(request_settings_change_request).unwrap();

        // Assert that the response contains an error
        assert!(response.error.is_some());
    }

    fn create_user_preferences_server_and_impl(
        mock_vsidl_client_provider: Weak<dyn VSIDLClientProvider>,
    ) -> UserPreferencesServiceImpl {
        let connection = Arc::new(Mutex::new(
            Connection::open_in_memory().expect("Failed to establish connection"),
        ));

        let user_repository = UserRepository::new(connection.clone())
            .expect("Failed to create an instance of user repository");

        let settings_group_repository = SettingsGroupRepository::new(connection.clone())
            .expect("Failed to create an instance of settings group repository");

        let settings_definition_repository = SettingDefinitionRepository::new(connection.clone())
            .expect("Failed to create an instance of settings definition repository");

        let user_settings_repository = UserSettingsRepository::new(connection.clone())
            .expect("Failed to create an instance of user settings repository");

        let vehicle_settings_repository = VehicleSettingsRepository::new(connection.clone())
            .expect("Failed to create an instance of vehicle state repository");

        UserPreferencesServiceImpl::new(UserPreferencesServiceImplArgs {
            user_repository,
            settings_group_repository,
            settings_definition_repository,
            user_settings_repository,
            vehicle_settings_repository,
            vsidl_client_provider: mock_vsidl_client_provider,
        })
    }

    fn default_bool_setting() -> SettingProto {
        let setting_and_constraint =
            default_bool_setting_definition().setting_and_constraints.unwrap();
        setting_and_constraint.setting.unwrap()
    }

    fn default_bool_setting_definition() -> SettingDefinitionProto {
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
        }
    }

    fn default_int32_setting() -> SettingProto {
        let setting_and_constraint =
            default_int32_setting_definition().setting_and_constraints.unwrap();
        setting_and_constraint.setting.unwrap()
    }

    fn default_int32_setting_definition() -> SettingDefinitionProto {
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
        }
    }

    fn are_settings_equal(list_one: &[SettingProto], list_two: &[SettingProto]) -> bool {
        list_one.iter().map(|x| (&x.key, &x.value)).collect::<HashMap<_, _>>()
            == list_two.iter().map(|x| (&x.key, &x.value)).collect::<HashMap<_, _>>()
    }
}
