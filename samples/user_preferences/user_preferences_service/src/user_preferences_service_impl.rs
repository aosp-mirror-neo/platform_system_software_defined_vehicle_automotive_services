// Copyright 2023 Google LLC
use anyhow::Result;
use itertools::Itertools;
use log::{debug, error, info, warn};
use std::collections::HashSet;

use sdvgenerated::sdv_user_preferences_user_controllable_user_controllable_service_interface::UserControllableService;
use sdvgenerated::sdv_user_preferences_user_preferences_admin_service_interface::UserPreferencesAdminService;
use sdvgenerated::sdv_user_preferences_user_preferences_management_service_interface::UserPreferencesManagementService;
use sdvgenerated::sdv_user_preferences_user_preferences_registry_service_interface::UserPreferencesRegistryService;
use sdvgenerated::sdv_user_preferences_view_change_notifier_interface::ChangeNotifier;
use user_preferences_api::user_preferences_management_service::request_settings_change_request::ChangePersistencePolicy;

use user_preferences_api::change_notifier::OnSettingsChangeRequest;
use user_preferences_api::error::{Error, ResponseCode};
use user_preferences_api::setting::Setting as SettingProto;
use user_preferences_api::setting::{
    setting::Value as ValueProto, SettingAndConstraints,
    SettingDefinition as SettingDefinitionProto, SettingKind,
};
use user_preferences_api::user::User as UserProto;
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
    subscribers_repository: Mutex<HashMap<i64, HashSet<String>>>,
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
            subscribers_repository: Mutex::new(HashMap::new()),
            active_user_id: Mutex::new(None),
        }
    }

    pub fn calculate_group_settings_for_vehicle_state(
        &self,
        settings_group_id: i64,
    ) -> Result<Vec<SettingAndConstraints>> {
        let default_settings = self
            .settings_definition_repository
            .get(settings_group_id)?
            .into_iter()
            .map(|setting_definition| SettingAndConstraints {
                setting: Some(SettingProto {
                    key: setting_definition.name.clone(),
                    value: Some(setting_definition.default_value.into()),
                    ..Default::default()
                })
                .into(),
                constraints: setting_definition.constraint.map(|s| s.into()),
                ..Default::default()
            })
            .collect_vec();

        let vehicle_state = self.vehicle_settings_repository.get(settings_group_id)?;
        let vehicle_state: HashMap<String, ValueProto> =
            vehicle_state.into_iter().map(|setting| (setting.name, setting.value.into())).collect();

        let effective_settings = default_settings
            .into_iter()
            .map(|mut setting_and_constraints| {
                let setting = setting_and_constraints.setting.as_mut().unwrap();
                if let Some(vehicle_value) = vehicle_state.get(&setting.key) {
                    setting.value = Some(vehicle_value.clone());
                }
                setting_and_constraints
            })
            .collect_vec();

        Ok(effective_settings)
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
        let client = self
            .vsidl_client_provider
            .upgrade()
            //This is fatal error since we are not able to communicate setting changes to user controllable services
            //nor notify subscribers of setting changes
            .expect("VSIDL client provider has been dropped")
            .get_sdv_user_preferences_user_controllable_user_controllable_service_client(
                &settings_group.service_fqin,
            );

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

    fn get_subscribers(&self, settings_group_id: i64) -> Vec<String> {
        let subscribers_repository = self.subscribers_repository.lock().unwrap();
        subscribers_repository
            .get(&settings_group_id)
            .unwrap_or(&HashSet::new())
            .iter()
            .cloned()
            .collect_vec()
    }

    fn notify_subscribers(
        &self,
        updated_setting_group: &SettingsGroup,
        updated_settings_name: &HashSet<String>,
    ) {
        let current_user_id: Option<i32> = *self.active_user_id.lock().unwrap();
        let current_settings_applied: Vec<SettingProto> =
            match self.vehicle_settings_repository.get(updated_setting_group.id) {
                Ok(settings) => settings
                    .into_iter()
                    .filter(|s| updated_settings_name.contains(&s.name))
                    .map_into()
                    .collect_vec(),
                Err(err) => {
                    error!("Failed to retrieve current vehicle settings: {}", err);
                    return;
                }
            };

        let persisted_settings: Vec<SettingProto> = match current_user_id {
            None => Vec::new(),
            Some(id) => {
                match self.calculate_group_settings_for_user(id, updated_setting_group.id) {
                    Ok(settings) => settings
                        .into_iter()
                        .filter(|s| updated_settings_name.contains(&s.name))
                        .map_into()
                        .collect_vec(),
                    Err(err) => {
                        error!("Failed to retrieve persisted user settings: {}", err);
                        return;
                    }
                }
            }
        };
        let subscribers = self.get_subscribers(updated_setting_group.id);

        for subscriber in subscribers {
            let client = self
                .vsidl_client_provider
                .upgrade()
                //This is fatal error since we are not able to communicate setting changes to user controllable services
                //nor notify subscribers of setting changes
                .expect("VSIDL client provider has been dropped")
                .get_sdv_user_preferences_view_change_notifier_client(&subscriber);

            info!("Notifying {} of settings change\n", subscriber);
            if let Err(err) = client.on_settings_change(OnSettingsChangeRequest {
                settings_group_id: MessageField::some(updated_setting_group.clone().into()),
                settings: current_settings_applied.clone(),
                //TODO: Expose pending settings for the user to subscribers
                persisted_settings: persisted_settings.clone(),
                ..Default::default()
            }) {
                error!("Failed to notify {} of settings change: {}", subscriber, err);
            }
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

            if let Err(err) = self
                .apply_settings(group.clone(), settings.clone().into_iter().map_into().collect())
            {
                error!(
                    "Failed to apply settings from settings group {}/{}: {}",
                    group.service_fqin, group.name, err
                );
            }

            self.notify_subscribers(group, &settings.into_iter().map(|x| x.name.clone()).collect());
        }

        Ok(SelectUserResponse::new())
    }

    fn delete_user(&self, request: DeleteUserRequest) -> RpcResult<DeleteUserResponse> {
        if Some(request.user_id) == *self.active_user_id.lock().unwrap() {
            return error_response!(
                DeleteUserResponse,
                ResponseCode::INVALID_REQUEST,
                "Cannot delete user {} it is currently set as the active user",
                request.user_id
            );
        }

        if let Err(err) = self.user_repository.delete(request.user_id) {
            error!("Failed to delete user {}: {}", request.user_id, err);
            match err {
                crate::persistence::error::Error::NotFound => {
                    return error_response!(
                        DeleteUserResponse,
                        ResponseCode::NOT_FOUND_ERROR,
                        "User not found",
                    )
                }
                _ => {
                    return error_response!(
                        DeleteUserResponse,
                        ResponseCode::INTERNAL_ERROR,
                        "Unable to delete user {}",
                        err
                    )
                }
            };
        }

        if let Err(err) = self.user_settings_repository.delete_all(request.user_id) {
            error!("Failed to delete settings of user {}: {}", request.user_id, err);
            return error_response!(
                DeleteUserResponse,
                ResponseCode::INTERNAL_ERROR,
                "Unable to delete user {}",
                request.user_id
            );
        }

        Ok(DeleteUserResponse::new())
    }

    fn factory_reset(&self, _request: FactoryResetRequest) -> RpcResult<FactoryResetResponse> {
        RpcResult::Err(sdvmiddleware_rpc::error::RpcError::Other)
    }

    fn list_users(&self, _request: ListUsersRequest) -> RpcResult<ListUsersResponse> {
        let users = match self.user_repository.list() {
            Ok(x) => x,
            Err(err) => {
                error!("Failed to list users: {}", err);
                return error_response!(
                    ListUsersResponse,
                    ResponseCode::INTERNAL_ERROR,
                    "Unable to list users",
                );
            }
        };

        let user_proto_list: Vec<UserProto> = users.into_iter().map_into().collect();

        Ok(ListUsersResponse { users: user_proto_list, ..Default::default() })
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
                        ResponseCode::NOT_FOUND_ERROR,
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

        self.notify_subscribers(
            &settings_group,
            &request.settings.into_iter().map(|x| x.key.clone()).collect(),
        );

        Ok(RequestSettingsChangeResponse::new())
    }

    fn subscribe_to_settings_change_and_get_settings(
        &self,
        request: SubscribeToSettingsChangeAndGetSettingsRequest,
    ) -> RpcResult<SubscribeToSettingsChangeAndGetSettingsResponse> {
        let settings_group_id = request.settings_group_id.unwrap();
        let settings_group = match self
            .settings_group_repository
            .get(settings_group_id.name.as_str(), settings_group_id.service_fqin.as_str())
        {
            Ok(x) => x,
            Err(err) => match err {
                crate::persistence::error::Error::NotFound => {
                    warn!(
                        "Setting group {}/{} was not found",
                        settings_group_id.service_fqin, settings_group_id.name
                    );
                    return error_response!(
                        SubscribeToSettingsChangeAndGetSettingsResponse,
                        ResponseCode::NOT_FOUND_ERROR,
                        "Setting group {}/{} was not found",
                        settings_group_id.service_fqin,
                        settings_group_id.name
                    );
                }
                _ => {
                    error!(
                        "Unable to retrieve setting group {}/{}: {}",
                        settings_group_id.service_fqin, settings_group_id.name, err
                    );
                    return error_response!(
                        SubscribeToSettingsChangeAndGetSettingsResponse,
                        ResponseCode::INTERNAL_ERROR,
                        "Unable to retrieve setting group {}/{}: {}",
                        settings_group_id.service_fqin,
                        settings_group_id.name,
                        err
                    );
                }
            },
        };

        let current_settings: Vec<SettingAndConstraints> =
            self.calculate_group_settings_for_vehicle_state(settings_group.id).unwrap();
        let persisted_settings: Vec<SettingProto> = match *self.active_user_id.lock().unwrap() {
            Some(user_id) => {
                match self.calculate_group_settings_for_user(user_id, settings_group.id) {
                    Ok(x) => x.into_iter().map_into().collect_vec(),
                    Err(err) => {
                        error!("Failed to retrieve persisted settings: {}", err);
                        return error_response!(
                            SubscribeToSettingsChangeAndGetSettingsResponse,
                            ResponseCode::INTERNAL_ERROR,
                            "Unable to retrieve setting group {}/{}: {}",
                            settings_group_id.service_fqin,
                            settings_group_id.name,
                            err
                        );
                    }
                }
            }
            None => Vec::new(),
        };

        let mut subscribers_repository = self.subscribers_repository.lock().unwrap();

        subscribers_repository.entry(settings_group.id).or_default().insert(request.caller_fqin);

        Ok(SubscribeToSettingsChangeAndGetSettingsResponse {
            current_settings,
            //TODO: Add version number to settings_group & return it here
            persisted_settings,
            ..Default::default()
        })
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
    use crate::test_utils::*;
    use crate::vsidl_client_provider::{MockClientTransport, MockVSIDLClientProvider};
    use protobuf::Message;
    use rusqlite::Connection;
    use sdvgenerated_rpc_sdv_user_preferences_user_controllable_user_controllable_service::client::UserControllableServiceClient;
    use sdvgenerated_rpc_sdv_user_preferences_view_change_notifier::client::ChangeNotifierClient;
    use std::sync::{Arc, Mutex};
    use user_preferences_api::change_notifier::OnSettingsChangeResponse;
    use user_preferences_api::setting::{setting::Value as ValueProto, Setting as SettingProto};
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
            group_name: TEST_SETTINGS_GROUP_NAME.to_string(),
            caller_fqin: TEST_USER_CONTROLLABLE_SERVICE_FQIN.to_string(),
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
                group_name: String::from(TEST_SETTINGS_GROUP_NAME),
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
                target_service_bundle == TEST_USER_CONTROLLABLE_SERVICE_FQIN
            })
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
            group_name: TEST_SETTINGS_GROUP_NAME.to_string(),
            caller_fqin: TEST_USER_CONTROLLABLE_SERVICE_FQIN.to_string(),
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
                group_name: String::from(TEST_SETTINGS_GROUP_NAME),
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
                    target_service_bundle == TEST_USER_CONTROLLABLE_SERVICE_FQIN
                );
                target_service_bundle == TEST_USER_CONTROLLABLE_SERVICE_FQIN
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
            group_name: TEST_SETTINGS_GROUP_NAME.to_string(),
            caller_fqin: TEST_USER_CONTROLLABLE_SERVICE_FQIN.to_string(),
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
                name: TEST_SETTINGS_GROUP_NAME.to_string(),
                service_fqin: TEST_USER_CONTROLLABLE_SERVICE_FQIN.to_string(),
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
            group_name: TEST_SETTINGS_GROUP_NAME.to_string(),
            caller_fqin: TEST_USER_CONTROLLABLE_SERVICE_FQIN.to_string(),
            settings_definitions,
            ..Default::default()
        };
        user_preferences_impl.register_settings(register_settings_request).unwrap();

        // Request settings change with invalid settings
        let request_settings_change_request = RequestSettingsChangeRequest {
            settings_group_id: Some(SettingsGroupIdProto {
                name: TEST_SETTINGS_GROUP_NAME.to_string(),
                service_fqin: TEST_USER_CONTROLLABLE_SERVICE_FQIN.to_string(),
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
        assert_eq!(
            response.error.unwrap().error_code.value(),
            ResponseCode::INVALID_REQUEST as i32
        );
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
                service_fqin: TEST_USER_CONTROLLABLE_SERVICE_FQIN.to_string(),
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
        assert_eq!(
            response.error.unwrap().error_code.value(),
            ResponseCode::NOT_FOUND_ERROR as i32
        );
    }

    #[test]
    fn subscribe_to_settings_change_and_get_settings_returns_error_for_non_existent_group() {
        let mock_vsidl_client_provider: Arc<dyn VSIDLClientProvider> =
            Arc::new(MockVSIDLClientProvider::new());
        let user_preferences_impl =
            create_user_preferences_server_and_impl(Arc::downgrade(&mock_vsidl_client_provider));

        let non_existent_group_id = SettingsGroupIdProto {
            name: "non_existent_group".to_string(),
            service_fqin: TEST_USER_CONTROLLABLE_SERVICE_FQIN.to_string(),
            ..Default::default()
        };
        let response = subscribe_to_settings_change_and_get_settings(
            &user_preferences_impl,
            non_existent_group_id,
        );

        // Assert that the response contains an error
        assert!(response.error.is_some());
        assert_eq!(
            response.error.unwrap().error_code.value(),
            ResponseCode::NOT_FOUND_ERROR as i32
        );
    }

    #[test]
    fn subscribe_to_settings_change_and_get_settings_returns_group_settings() {
        let user_controllable_service_client = Arc::new(UserControllableServiceClient::new({
            let mut mock_client_transport = Box::new(MockClientTransport::new());

            let rpc_response =
                user_preferences_api::user_controllable_service::RequestSettingsChangeResponse {
                    settings: vec![default_int32_setting(), default_bool_setting()],
                    ..Default::default()
                };

            mock_client_transport
                .expect_call_method()
                .times(1)
                .returning(move |_, _| Ok(rpc_response.write_to_bytes().unwrap()));

            mock_client_transport
        }));

        let mock_vsidl_client_provider: Arc<dyn VSIDLClientProvider> = Arc::new({
            let mut mock_vsidl_client_provider = MockVSIDLClientProvider::new();

            mock_vsidl_client_provider
                .expect_get_sdv_user_preferences_user_controllable_user_controllable_service_client(
                )
                .times(1)
                .returning(move |_| user_controllable_service_client.clone());

            mock_vsidl_client_provider
        });
        let user_preferences_impl = create_user_preferences_server_with_default_settings_and_user(
            Arc::downgrade(&mock_vsidl_client_provider),
        );

        let response = subscribe_to_settings_change_and_get_settings(
            &user_preferences_impl,
            DEFAULT_TEST_SETTING_GROUP_ID.clone(),
        );

        assert!(response.error.is_none());
        assert_eq!(response.current_settings.len(), 2);
        assert_eq!(response.persisted_settings.len(), 2)
    }

    #[test]
    fn request_settings_change_notifies_subscribers_of_setting_changes() {
        let change_notifier_client = Arc::new(ChangeNotifierClient::new({
            let mut mock_client_transport = Box::new(MockClientTransport::new());

            let expected_rpc_request = OnSettingsChangeRequest {
                settings_group_id: Some(SettingsGroupIdProto {
                    name: TEST_SETTINGS_GROUP_NAME.to_string(),
                    service_fqin: TEST_USER_CONTROLLABLE_SERVICE_FQIN.to_string(),
                    ..Default::default()
                })
                .into(),
                version: String::from(""),
                settings: vec![SettingProto {
                    key: "int32_setting".to_string(),
                    value: Some(ValueProto::Int32(20)),
                    ..Default::default()
                }],
                persisted_settings: vec![SettingProto {
                    key: "int32_setting".to_string(),
                    value: Some(ValueProto::Int32(10)),
                    ..Default::default()
                }],
                ..Default::default()
            };

            let rpc_response = OnSettingsChangeResponse { ..Default::default() };

            mock_client_transport
                .expect_call_method()
                .times(1)
                .withf(move |_method, request| {
                    debug!("request_settings_change_notifies_subscribers_of_setting_changes");
                    let request =
                        OnSettingsChangeRequest::parse_from_bytes(
                            request,
                        )
                        .unwrap();
                    debug!("request_settings_change_notifies_subscribers_of_setting_changes request {:?}", request);
                    request.settings_group_id.eq(&expected_rpc_request.settings_group_id)
                        && request.version.eq(&expected_rpc_request.version)
                        && are_settings_equal(&request.settings, &expected_rpc_request.settings)
                        && are_settings_equal(&request.persisted_settings, &expected_rpc_request.persisted_settings)
                })
                .returning(move |_, _| Ok(rpc_response.write_to_bytes().unwrap()));

            mock_client_transport
        }));

        let user_controllable_service_client = Arc::new(UserControllableServiceClient::new({
            let mut mock_client_transport = Box::new(MockClientTransport::new());

            let rpc_response =
                user_preferences_api::user_controllable_service::RequestSettingsChangeResponse {
                    settings: vec![SettingProto {
                        key: "int32_setting".to_string(),
                        value: Some(ValueProto::Int32(20)),
                        ..Default::default()
                    }],
                    ..Default::default()
                };

            mock_client_transport
                .expect_call_method()
                .times(2)
                .returning(move |_, _| Ok(rpc_response.write_to_bytes().unwrap()));

            mock_client_transport
        }));

        let mock_vsidl_client_provider: Arc<dyn VSIDLClientProvider> = Arc::new({
            let mut mock_vsidl_client_provider = MockVSIDLClientProvider::new();

            mock_vsidl_client_provider
                .expect_get_sdv_user_preferences_user_controllable_user_controllable_service_client(
                )
                .times(2)
                .returning(move |_| user_controllable_service_client.clone());

            mock_vsidl_client_provider
                .expect_get_sdv_user_preferences_view_change_notifier_client()
                .times(1)
                .returning(move |_| change_notifier_client.clone());

            mock_vsidl_client_provider
        });
        let user_preferences_impl = create_user_preferences_server_with_default_settings_and_user(
            Arc::downgrade(&mock_vsidl_client_provider),
        );

        // Subscribe to settings change and get settings
        let _ = subscribe_to_settings_change_and_get_settings(
            &user_preferences_impl,
            DEFAULT_TEST_SETTING_GROUP_ID.clone(),
        );

        // Request settings change with valid settings
        let request_settings_change_request = RequestSettingsChangeRequest {
            settings_group_id: Some(SettingsGroupIdProto {
                name: TEST_SETTINGS_GROUP_NAME.to_string(),
                service_fqin: TEST_USER_CONTROLLABLE_SERVICE_FQIN.to_string(),
                ..Default::default()
            })
            .into(),
            settings: vec![SettingProto {
                key: "int32_setting".to_string(),
                value: Some(ValueProto::Int32(20)),
                ..Default::default()
            }],
            change_persistence_policy: ChangePersistencePolicy::NON_PERSISTENT_CHANGE.into(),
            ..Default::default()
        };
        let _response =
            user_preferences_impl.request_settings_change(request_settings_change_request).unwrap();
    }

    fn create_user_preferences_server_with_default_settings_and_user(
        mock_vsidl_client_provider: Weak<dyn VSIDLClientProvider>,
    ) -> UserPreferencesServiceImpl {
        let user_preferences_impl =
            create_user_preferences_server_and_impl(mock_vsidl_client_provider);

        register_default_settings_and_assert_success(&user_preferences_impl);

        let user_id = 1_i32;

        create_user_and_assert_success(&user_preferences_impl, user_id, 0);

        select_user_and_assert_success(&user_preferences_impl, user_id);

        user_preferences_impl
    }

    #[test]
    fn delete_user_succeeds() {
        let mock_vsidl_client_provider: Arc<dyn VSIDLClientProvider> =
            Arc::new(MockVSIDLClientProvider::new());
        let user_preferences_impl =
            create_user_preferences_server_and_impl(Arc::downgrade(&mock_vsidl_client_provider));

        // Create a user
        let user_id = 1_i32;
        create_user_and_assert_success(&user_preferences_impl, user_id, 0);

        // Delete the user
        let delete_user_request = DeleteUserRequest { user_id, ..Default::default() };
        let response = user_preferences_impl.delete_user(delete_user_request).unwrap();

        // Assert that the response is ok
        assert!(response.error.is_none());
    }

    #[test]
    fn delete_user_fails_when_user_does_not_exist() {
        let mock_vsidl_client_provider: Arc<dyn VSIDLClientProvider> =
            Arc::new(MockVSIDLClientProvider::new());
        let user_preferences_impl =
            create_user_preferences_server_and_impl(Arc::downgrade(&mock_vsidl_client_provider));

        // Attempt to delete a non-existent user
        let delete_user_request = DeleteUserRequest { user_id: 1, ..Default::default() };
        let response = user_preferences_impl.delete_user(delete_user_request).unwrap();

        // Assert that the response contains an error
        assert!(response.error.is_some());
        assert_eq!(
            response.error.unwrap().error_code.value(),
            ResponseCode::NOT_FOUND_ERROR as i32
        );
    }

    #[test]
    fn delete_user_fails_when_user_is_selected() {
        let mock_vsidl_client_provider: Arc<dyn VSIDLClientProvider> =
            Arc::new(MockVSIDLClientProvider::new());
        let user_preferences_impl =
            create_user_preferences_server_and_impl(Arc::downgrade(&mock_vsidl_client_provider));

        let user_id = 1_i32;
        create_user_and_assert_success(&user_preferences_impl, user_id, 0);

        select_user_and_assert_success(&user_preferences_impl, user_id);

        let delete_user_request = DeleteUserRequest { user_id: 1, ..Default::default() };
        let response = user_preferences_impl.delete_user(delete_user_request).unwrap();

        assert!(response.error.is_some());
        assert_eq!(
            response.error.unwrap().error_code.value(),
            ResponseCode::INVALID_REQUEST as i32
        );
    }

    #[test]
    fn list_users_returns_all_users() {
        let mock_vsidl_client_provider: Arc<dyn VSIDLClientProvider> =
            Arc::new(MockVSIDLClientProvider::new());
        let user_preferences_impl =
            create_user_preferences_server_and_impl(Arc::downgrade(&mock_vsidl_client_provider));

        let users = [
            UserProto { id: 1, flags: 0, ..Default::default() },
            UserProto { id: 2, flags: 1, ..Default::default() },
            UserProto { id: 3, flags: 2, ..Default::default() },
        ];
        for user in users.iter() {
            let request =
                CreateUserRequest { user: MessageField::some(user.clone()), ..Default::default() };
            user_preferences_impl.create_user(request).unwrap();
        }

        let mut response = user_preferences_impl.list_users(ListUsersRequest::new()).unwrap();
        response.users.sort_by(|a, b| a.id.cmp(&b.id));

        assert_eq!(response.users.len(), 3);
        assert_eq!(response.users[0].id, 1);
        assert_eq!(response.users[1].id, 2);
        assert_eq!(response.users[2].id, 3);
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

    fn are_settings_equal(list_one: &[SettingProto], list_two: &[SettingProto]) -> bool {
        list_one.iter().map(|x| (&x.key, &x.value)).collect::<HashMap<_, _>>()
            == list_two.iter().map(|x| (&x.key, &x.value)).collect::<HashMap<_, _>>()
    }
}
