// Copyright 2023 Google LLC

use itertools::Itertools;
use std::collections::HashSet;

use sdvgenerated::sdv_user_preferences_user_preferences_admin_service_interface::UserPreferencesAdminService;
use sdvgenerated::sdv_user_preferences_user_preferences_management_service_interface::UserPreferencesManagementService;
use sdvgenerated::sdv_user_preferences_user_preferences_registry_service_interface::UserPreferencesRegistryService;
use sdvgenerated_rpc_sdv_user_preferences_user_controllable_user_controllable_service::client::UserControllableServiceClient;
use sdvgenerated_rpc_sdv_user_preferences_user_controllable_user_controllable_service::interface::UserControllableService;
use sdvgenerated_rpc_sdv_user_preferences_user_preferences_view::interface::UserPreferencesView;

use sdvgenerated_rpc_sdv_user_preferences_user_preferences_view::client::UserPreferencesViewClient;
use sdvmiddleware_rpc_grpc::grpc_client_transport::GrpcClientTransport;

use user_preferences_api::settings_group::SettingsGroupId as SettingsGroupIdProto;
use user_preferences_api::user_preferences_management_service::request_settings_change_request::ChangePersistencePolicy;
use user_preferences_api::user_preferences_registry_service::{
    RegisterSettingsRequest, RegisterSettingsResponse,
};

use user_preferences_api::error::{Error, ResponseCode};
use user_preferences_api::setting::{Setting as SettingProto, SettingKind};
use user_preferences_api::user_preferences_admin_service::*;
use user_preferences_api::user_preferences_management_service::*;
use user_preferences_api::user_preferences_registry_service::{
    UpdateSettingsRequest, UpdateSettingsResponse,
};

use protobuf::{EnumOrUnknown, MessageField};
use sdvmiddleware_rpc::RpcResult;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::vec::Vec;
use user_preferences_api::user_preferences_view::OnSettingsChangeRequest;

use user_preferences_api::user_controllable_service;

use crate::macros::{acquire_lock_or_return_err, error_response};
use crate::setting::Setting;
use crate::settings_group::SettingsGroup;
use crate::settings_group_id::SettingsGroupId;
use crate::settings_store::SettingsStore;
use crate::user_manager::UserManager;
use user_preferences_utils::{print_setting_definition, print_settings};

pub struct UserPreferencesServiceImpl {
    vehicle_state: Mutex<SettingsStore>,
    user_manager: UserManager,
    current_user_id: Mutex<Option<i32>>,
    change_subscribers: Mutex<HashMap<SettingsGroupId, HashSet<String>>>,
}

impl UserPreferencesServiceImpl {
    pub fn new() -> Self {
        UserPreferencesServiceImpl {
            vehicle_state: Mutex::new(SettingsStore::new()),
            user_manager: UserManager::new(),
            current_user_id: Mutex::new(None),
            change_subscribers: Mutex::new(HashMap::new()),
        }
    }

    fn get_settings_group_from_current_user(
        &self,
        settings_group_id: &SettingsGroupId,
    ) -> Option<Arc<SettingsGroup>> {
        let current_user_id = self.current_user_id.lock().unwrap();

        current_user_id.and_then(|id| self.user_manager.get_user(id)).and_then(|user| {
            user.vehicle_configuration.lock().unwrap().get_group(settings_group_id)
        })
    }

    fn persist_to_current_user(
        &self,
        settings_group_id: SettingsGroupId,
        new_settings: Vec<SettingProto>,
    ) -> Result<(), Error> {
        let current_user_id = self.current_user_id.lock().unwrap();

        let user = current_user_id.and_then(|id| self.user_manager.get_user(id));

        let user = match user {
            Some(user) => user,
            None => {
                return Err(Error {
                    error_code: ResponseCode::INVALID_REQUEST.into(),
                    message: String::from("Could not find the user to persist the settings within"),
                    ..Default::default()
                })
            }
        };
        let user_persisted_settings = user.vehicle_configuration.lock().unwrap();
        let persisted_settings = user_persisted_settings.get_group(&settings_group_id).unwrap();

        persisted_settings.update(new_settings)
    }

    fn get_subscribers(&self, settings_group_id: &SettingsGroupId) -> Result<Vec<String>, Error> {
        let change_subscribers = self.change_subscribers.lock().unwrap();
        Ok(change_subscribers
            .get(settings_group_id)
            .unwrap_or(&HashSet::new())
            .iter()
            .cloned()
            .collect_vec())
    }

    fn notify_subscribers(
        &self,
        applied_settings_group: Arc<SettingsGroup>,
        persisted_settings_group: Option<Arc<SettingsGroup>>,
        changed_settings: &[String],
    ) -> Result<(), Error> {
        let current_settings_applied: Vec<SettingProto> = changed_settings
            .iter()
            .filter_map(|name| applied_settings_group.get(name))
            .map(Setting::into)
            .collect_vec();
        let persisted_settings: Vec<SettingProto> = match persisted_settings_group {
            None => vec![],
            Some(persisted_settings_group) => changed_settings
                .iter()
                .filter_map(|name| persisted_settings_group.get(name))
                .map(Setting::into)
                .collect_vec(),
        };
        let subscribers = self.get_subscribers(&applied_settings_group.id)?;

        // Ideally the client should be created via the get_user_controllable_service_client method
        // that is present in the UserPreferencesServer, for now manually create it since the rust
        // compiler cannot determine the correctness of the program if UserPreferencesServiceImpl & UserPreferencesServer
        // have references to each other
        for subscriber in subscribers {
            let client =
                UserPreferencesViewClient::new(Box::new(GrpcClientTransport::new(&subscriber)));

            println!("Notifying {} of settings change\n", subscriber);
            client
                .on_settings_change(OnSettingsChangeRequest {
                    settings_group_id: MessageField::some(applied_settings_group.id.clone().into()),
                    settings: current_settings_applied.clone(),
                    //TODO: Expose pending settings for the user to subscribers
                    persisted_settings: persisted_settings.clone(),
                    ..Default::default()
                })
                .expect("Failed to notify clients of settings changes");
        }

        Ok(())
    }

    fn apply_settings(
        settings_group_id: SettingsGroupIdProto,
        settings: Vec<SettingProto>,
    ) -> Result<(), String> {
        // Ideally the client should be created via the get_user_controllable_service_client method
        // that is present in the UserPreferencesServer, for now manually create it since the rust
        // compiler cannot determine the correctness of the program if UserPreferencesServiceImpl & UserPreferencesServer
        // have references to each other
        let client: UserControllableServiceClient = UserControllableServiceClient::new(Box::new(
            GrpcClientTransport::new(&settings_group_id.service_fqin),
        ));

        let resp = client
            .request_settings_change(user_controllable_service::RequestSettingsChangeRequest {
                group_name: settings_group_id.name,
                settings,
                ..Default::default()
            })
            .map_err(|x| {
                format!("Failed to perform GRPC request to request_settings_change: {}", x)
            })?;

        match resp.error.is_some() {
            true => Err(format!("Failed to request settings changes: {}", resp.error.unwrap())),
            false => Ok(()),
        }
    }

    fn reset_services(services_fqin: Vec<String>) -> Result<(), String> {
        for service_fqin in services_fqin {
            let client = UserControllableServiceClient::new(Box::new(GrpcClientTransport::new(
                &service_fqin,
            )));

            println!("Factory resetting {}", service_fqin);
            let resp = client
                .factory_reset(user_controllable_service::FactoryResetRequest::new())
                .map_err(|x| {
                    format!("Failed to perform GRPC request to request_settings_change: {}", x)
                })?;

            if resp.error.is_some() {
                return Err(format!(
                    "Failed to factory reset {} service: {}",
                    service_fqin,
                    resp.error.unwrap()
                ));
            }
        }
        Ok(())
    }

    fn validate_requested_setting_changes(
        &self,
        current_settings: &SettingsGroup,
        requested_setting_changes: &[SettingProto],
    ) -> Result<(), String> {
        for setting in requested_setting_changes.iter() {
            let current_settings = match current_settings.get(&setting.key) {
                Some(x) => x,
                None => {
                    return Err(format!(
                        "Setting {} cannot be found within setting group {}",
                        setting.key, current_settings.id
                    ))
                }
            };
            if let Some(value) = &setting.value {
                if !current_settings.is_valid_value(value) {
                    return Err(format!(
                        "{:#?} is not a valid value for Setting {}",
                        value, setting.key
                    ));
                }
            } else {
                return Err(format!("Setting {} has None value", setting.key));
            }
        }

        Ok(())
    }
}

impl UserPreferencesAdminService for UserPreferencesServiceImpl {
    fn create_user(&self, request: CreateUserRequest) -> RpcResult<CreateUserResponse> {
        if request.user.is_none() {
            println!("Invalid create user request, user field cannot be empty");
            return error_response!(
                CreateUserResponse,
                ResponseCode::INVALID_REQUEST,
                "Invalid create user request, user field cannot be empty",
            );
        }
        let user = request.user.unwrap();

        // Initialize user with default values
        let vehicle_state = acquire_lock_or_return_err!(self.vehicle_state, CreateUserResponse);
        let mut user_persisted_configuration = vehicle_state.clone();
        user_persisted_configuration.reset();

        println!("Created user {}", user.id);

        match self.user_manager.create_user(user, user_persisted_configuration) {
            Ok(_) => Ok(CreateUserResponse::new()),
            Err(err) => Ok(CreateUserResponse { error: Some(err).into(), ..Default::default() }),
        }
    }

    fn select_user(&self, request: SelectUserRequest) -> RpcResult<SelectUserResponse> {
        println!("Loading user {}\n", request.user_id);

        let user = match self.user_manager.get_user(request.user_id) {
            Some(x) => x,
            None => {
                // return error_response!();
                return error_response!(
                    SelectUserResponse,
                    ResponseCode::INVALID_REQUEST,
                    "User {} does not exist",
                    request.user_id
                );
            }
        };

        // If the previous user is ephemeral, then delete them
        let mut current_user_id = self.current_user_id.lock().unwrap();
        if let Some(previous_user) = current_user_id.and_then(|id| self.user_manager.get_user(id)) {
            if previous_user.is_ephemeral() {
                if let Err(err) = self.user_manager.delete_user(previous_user.id) {
                    return Ok(SelectUserResponse {
                        error: Some(err).into(),
                        ..Default::default()
                    });
                }
                println!("Deleted ephemeral user: {}", previous_user.id);
            }
        }
        // Specify new user
        *current_user_id = Some(request.user_id);
        drop(current_user_id);

        //TODO(b/340806032): Switch to async request once middleware supports async clients
        let user_persisted_setting_groups =
            user.vehicle_configuration.lock().unwrap().get_all_groups();

        for settings_group in user_persisted_setting_groups.into_iter() {
            let settings = settings_group
                .get_all()
                .into_iter()
                .filter(|x| x.kind.as_ref() == &SettingKind::PER_USER)
                .map(SettingProto::from)
                .collect_vec();

            // TODO, also handle non RPC error
            match self.request_settings_change(RequestSettingsChangeRequest {
                settings_group_id: Some(settings_group.id.clone().into()).into(),
                settings,
                // Non persist as we are simply applying changes that are already persisted
                change_persistence_policy: ChangePersistencePolicy::NON_PERSISTENT_CHANGE.into(),
                ..Default::default()
            }) {
                Ok(response) => {
                    if let Some(err) = response.error.into_option() {
                        println!("Failed to apply settings changes: {}", err)
                    }
                }
                Err(err) => println!("Failed to apply settings changes: {}", err),
            };
        }
        //END of async part

        Ok(SelectUserResponse::new())
    }

    fn delete_user(&self, request: DeleteUserRequest) -> RpcResult<DeleteUserResponse> {
        Ok(DeleteUserResponse {
            error: self.user_manager.delete_user(request.user_id).err().into(),
            ..Default::default()
        })
    }

    fn factory_reset(&self, _request: FactoryResetRequest) -> RpcResult<FactoryResetResponse> {
        self.user_manager.delete_all();

        let mut current_user_id =
            acquire_lock_or_return_err!(self.current_user_id, FactoryResetResponse);
        *current_user_id = None;
        drop(current_user_id);

        let services_fqin = self
            .vehicle_state
            .lock()
            .unwrap()
            .get_all_groups()
            .iter()
            .map(|x| x.id.service_fqin().clone())
            .collect_vec();

        let resp = match Self::reset_services(services_fqin) {
            Ok(_) => FactoryResetResponse::new(),
            Err(err) => FactoryResetResponse {
                error: MessageField::some(Error {
                    error_code: ResponseCode::INTERNAL_ERROR.into(),
                    message: err,
                    ..Default::default()
                }),
                ..Default::default()
            },
        };

        Ok(resp)
    }

    fn list_users(&self, _request: ListUsersRequest) -> RpcResult<ListUsersResponse> {
        let users = self.user_manager.list_users();

        Ok(ListUsersResponse { users, ..Default::default() })
    }

    fn get_user_settings(
        &self,
        request: GetUserSettingsRequest,
    ) -> RpcResult<GetUserSettingsResponse> {
        let user = match self.user_manager.get_user(request.user_id) {
            Some(x) => x,
            None => {
                return error_response!(
                    GetUserSettingsResponse,
                    ResponseCode::INVALID_REQUEST,
                    "User {} does not exist",
                    request.user_id
                )
            }
        };
        let user_settings_store = user.vehicle_configuration.lock().unwrap();
        Ok(GetUserSettingsResponse {
            groups: user_settings_store
                .get_all_groups()
                .into_iter()
                .map(|s| s.as_ref().clone())
                .map_into()
                .collect_vec(),
            ..Default::default()
        })
    }
}

impl UserPreferencesManagementService for UserPreferencesServiceImpl {
    fn request_settings_change(
        &self,
        request: RequestSettingsChangeRequest,
    ) -> RpcResult<RequestSettingsChangeResponse> {
        let vehicle_state =
            acquire_lock_or_return_err!(self.vehicle_state, RequestSettingsChangeResponse);

        let settings_group_id: SettingsGroupId = request.settings_group_id.unwrap().into();

        let current_settings = match vehicle_state.get_group(&settings_group_id) {
            Some(settings) => settings,
            None => {
                return error_response!(
                    RequestSettingsChangeResponse,
                    ResponseCode::INVALID_REQUEST,
                    "Settings group {} not found",
                    settings_group_id
                )
            }
        };

        if let Err(err) =
            self.validate_requested_setting_changes(current_settings.as_ref(), &request.settings)
        {
            return error_response!(
                RequestSettingsChangeResponse,
                ResponseCode::INVALID_REQUEST,
                "{}",
                err,
            );
        }

        if request.change_persistence_policy.enum_value_or_default()
            == ChangePersistencePolicy::PERSISTENT_CHANGE
        {
            if let Err(err) =
                self.persist_to_current_user(settings_group_id.clone(), request.settings.clone())
            {
                return Ok(RequestSettingsChangeResponse {
                    error: MessageField::some(err),
                    ..Default::default()
                });
            };
        }

        println!("Updating the following settings within {}:", settings_group_id);
        print_settings(&request.settings);

        //TODO(b/340806032): Switch to async request once middleware supports async clients
        let resp = match Self::apply_settings(
            settings_group_id.clone().into(),
            request.settings.clone(),
        ) {
            Ok(_) => RequestSettingsChangeResponse::new(),
            Err(err) => {
                return error_response!(
                    RequestSettingsChangeResponse,
                    ResponseCode::RUNTIME_ERROR,
                    "{}",
                    err,
                )
            }
        };

        if let Err(err) = current_settings.update(request.settings.clone()) {
            println!("Failed to update internal state due to {}", err.message);
        }

        let persisted_group = self.get_settings_group_from_current_user(&settings_group_id);
        if let Err(err) = self.notify_subscribers(
            current_settings,
            persisted_group,
            &request.settings.into_iter().map(|x| x.key.clone()).collect_vec(),
        ) {
            println!("Failed to notify subscribers of settings change due to {}", err);
        }
        //End of async code block

        Ok(resp)
    }

    fn subscribe_to_settings_change_and_get_settings(
        &self,
        request: SubscribeToSettingsChangeAndGetSettingsRequest,
    ) -> RpcResult<SubscribeToSettingsChangeAndGetSettingsResponse> {
        let vehicle_state = acquire_lock_or_return_err!(
            self.vehicle_state,
            SubscribeToSettingsChangeAndGetSettingsResponse
        );
        let mut change_subscribers = acquire_lock_or_return_err!(
            self.change_subscribers,
            SubscribeToSettingsChangeAndGetSettingsResponse
        );
        let settings_group_id: SettingsGroupId = request.settings_group_id.unwrap().into();

        change_subscribers
            .entry(settings_group_id.clone())
            .or_default()
            .insert(request.caller_fqin);

        if let Some(settings_group) = vehicle_state.get_group(&settings_group_id) {
            let result = SubscribeToSettingsChangeAndGetSettingsResponse {
                current_settings: settings_group.get_all().into_iter().map_into().collect(),
                version: settings_group.version.clone(),
                ..Default::default()
            };

            println!("Successfully registered subscriber to settings: {}\n", settings_group_id);
            return Ok(result);
        }

        println!("Received request to subscribe to non existent settings: {}", settings_group_id);

        error_response!(
            SubscribeToSettingsChangeAndGetSettingsResponse,
            ResponseCode::INVALID_REQUEST,
            "Settings {} does not exist",
            settings_group_id
        )
    }

    fn unsubscribe_from_settings_change(
        &self,
        request: UnsubscribeToSettingsChangeRequest,
    ) -> RpcResult<UnsubscribeToSettingsChangeResponse> {
        let mut change_subscribers = acquire_lock_or_return_err!(
            self.change_subscribers,
            UnsubscribeToSettingsChangeResponse
        );

        let settings_group_id: SettingsGroupId = request.settings_group_id.unwrap().into();

        if let Some(settings_group_subscribers) = change_subscribers.get_mut(&settings_group_id) {
            settings_group_subscribers.remove(&request.caller_fqin);
        } else {
            return error_response!(
                UnsubscribeToSettingsChangeResponse,
                ResponseCode::INVALID_REQUEST,
                "Settings group {} does not exist",
                settings_group_id
            );
        }

        Ok(UnsubscribeToSettingsChangeResponse::new())
    }
}

impl UserPreferencesRegistryService for UserPreferencesServiceImpl {
    fn register_settings(
        &self,
        request: RegisterSettingsRequest,
    ) -> RpcResult<RegisterSettingsResponse> {
        let vehicle_state =
            acquire_lock_or_return_err!(self.vehicle_state, RegisterSettingsResponse);

        let settings_group_id =
            SettingsGroupId::new(request.caller_fqin.clone(), request.group_name.clone());

        let existing_setting_group = vehicle_state.get_group(&settings_group_id);

        if existing_setting_group.is_none()
            || existing_setting_group.unwrap().version != request.version
        {
            println!("Registered settings {}", settings_group_id);

            print_setting_definition(&request.settings_definitions);

            let settings: Vec<Setting> =
                request.settings_definitions.iter().cloned().map_into().collect_vec();
            vehicle_state
                .add_group(SettingsGroup::new(
                    settings_group_id.clone(),
                    request.version.clone(),
                    settings,
                ))
                .unwrap();
        } else {
            println!(
                "Settings group {} with version {} is already registered",
                settings_group_id, request.version
            );
        }

        Ok(RegisterSettingsResponse::new())
    }

    fn update_settings(&self, request: UpdateSettingsRequest) -> RpcResult<UpdateSettingsResponse> {
        let vehicle_state = acquire_lock_or_return_err!(self.vehicle_state, UpdateSettingsResponse);

        let settings_group_id = SettingsGroupId::new(request.caller_fqin, request.group_name);

        let service_settings_group = match vehicle_state.get_group(&settings_group_id) {
            Some(x) => x,
            None => {
                return error_response!(
                    UpdateSettingsResponse,
                    ResponseCode::NOT_FOUND_ERROR,
                    "Settings group {} not found",
                    settings_group_id
                )
            }
        };

        if let Err(err) = service_settings_group.update(request.settings.clone()) {
            return Ok(UpdateSettingsResponse { error: Some(err).into(), ..Default::default() });
        }
        println!("Updated the following settings");
        print_settings(&request.settings);

        let persisted_group = self.get_settings_group_from_current_user(&settings_group_id);

        if let Err(err) = self.notify_subscribers(
            service_settings_group,
            persisted_group,
            &request.settings.into_iter().map(|x| x.key.clone()).collect_vec(),
        ) {
            println!("Failed to notify subscribers of settings change due to {}", err);
        }

        Ok(UpdateSettingsResponse::new())
    }
}
