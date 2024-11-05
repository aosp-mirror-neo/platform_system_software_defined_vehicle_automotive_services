// Copyright 2024 Google LLC
//! Sample user preferences implementation
use com_sdv_google_user_preferences_user_preferences_admin_service_rpc::Interface as AdminService;

use crate::persistence::user_repository::User;
use crate::persistence::Persistence;
use crate::rpc_client_factory::RpcClientFactory;
use crate::settings_change_propagator_impl::SettingsChangePropagator;
use crate::settings_retrieval_utils::retrieve_user_settings_including_defaults_for_group;
use async_trait::async_trait;
use itertools::Itertools;
use log::{debug, error, info, warn};
use protobuf::Enum;
use sdv_comms::id::ServiceFqin;
use sdv_status_rs::{SdvResult, SdvStatus, SdvStatusCode};
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;
use user_preferences_api::settings_group::{
    SettingsGroup as SettingsGroupProto, SettingsGroupId as SettingsGroupIdProto,
};
use user_preferences_api::user::user::UserFlags;
use user_preferences_api::user::User as UserProto;
use user_preferences_api::user_controllable_service;
use user_preferences_api::user_preferences_admin_service::*;

pub struct AdminServiceImpl {
    persistence: Arc<Persistence>,
    rpc_client_factory: Arc<dyn RpcClientFactory>,
    settings_change_propagator: Arc<dyn SettingsChangePropagator>,
}

impl AdminServiceImpl {
    pub fn new(
        persistence: Arc<Persistence>,
        rpc_client_factory: Arc<dyn RpcClientFactory>,
        settings_change_propagator: Arc<dyn SettingsChangePropagator>,
    ) -> Self {
        Self { persistence, rpc_client_factory, settings_change_propagator }
    }

    fn are_valid_user_flags(user_flags: i32) -> bool {
        // Ensure driver flag is set since only driver is supported now
        if (user_flags & UserFlags::DRIVER.value()) != UserFlags::DRIVER.value() {
            warn!("User flags {} does not have driver flag set", user_flags);
            return false;
        }
        // Ensure that no other bits are set other than ephemeral & driver
        if user_flags > (UserFlags::EPHEMERAL.value() | UserFlags::DRIVER.value()) {
            warn!("User flags {} has unexpected bits set", user_flags);
            return false;
        }
        true
    }
}

#[async_trait]
impl AdminService for AdminServiceImpl {
    async fn CreateUser(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        request: &CreateUserRequest,
    ) -> SdvResult<CreateUserResponse> {
        info!("Request to create user: {:?}", request);

        let user: User = match request.user.clone().into_option() {
            Some(x) => x.into(),
            None => {
                return Err(SdvStatus::with_message(
                    SdvStatusCode::InvalidArgument,
                    "User cannot be empty",
                ));
            }
        };

        if !Self::are_valid_user_flags(user.flags) {
            return Err(SdvStatus::with_message(
                SdvStatusCode::InvalidArgument,
                format!("User flags {} is not a valid value", user.flags),
            ));
        }
        self.persistence.users.add_user(&user)?;

        info!("Successfully added user: {:?}", user);
        Ok(CreateUserResponse::new())
    }

    async fn SelectUser(
        &self,
        caller_id: sdv_comms::id::ServiceFqin,
        request: &SelectUserRequest,
    ) -> SdvResult<SelectUserResponse> {
        let previous_user = self.persistence.users.get_active_user()?;
        if previous_user.is_some_and(|user| user.id == request.user_id) {
            return Ok(SelectUserResponse::new());
        }

        let next_user = self.persistence.users.get_user(request.user_id)?;
        self.persistence.users.set_active_user(next_user.id)?;

        if previous_user.is_some_and(|user| user.is_ephemeral()) {
            if let Err(err) = self
                .DeleteUser(
                    caller_id,
                    &DeleteUserRequest {
                        user_id: previous_user.as_ref().unwrap().id,
                        ..Default::default()
                    },
                )
                .await
            {
                // Since deletion failed, revert the active_user user change
                error!(
                    "Failed to delete ephemeral user {:?}: {}",
                    previous_user.as_ref().unwrap(),
                    err
                );
                self.persistence.users.set_active_user(previous_user.as_ref().unwrap().id)?;
                return Ok(SelectUserResponse { ..Default::default() });
            }
            info!("Deleted ephemeral user: {}", previous_user.as_ref().unwrap().id);
        }

        info!("Switching to user {}", next_user.id);
        let groups = self.persistence.setting_groups.list()?;

        for group in groups.iter() {
            let settings = retrieve_user_settings_including_defaults_for_group(
                &self.persistence.setting_definitions,
                &self.persistence.user_settings,
                next_user.id,
                group.id,
            )
            .unwrap();

            debug!(
                "Loaded settings for group {:?}\n Applying the following settings: {:?}",
                group, settings
            );

            self.settings_change_propagator
                .queue_setting_change(
                    ServiceFqin::from_str(&group.service_fqin).unwrap(),
                    group.name.clone(),
                    settings.iter().cloned().map_into().collect(),
                )
                .await;
        }

        Ok(SelectUserResponse::new())
    }

    async fn DeleteUser(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        request: &DeleteUserRequest,
    ) -> SdvResult<DeleteUserResponse> {
        let active_user = match self.persistence.users.get_active_user() {
            Ok(user) => user,
            Err(err) => {
                error!("Failed to retrieve active user: {:?}", err);
                return Err(SdvStatus::with_message(
                    SdvStatusCode::Internal,
                    format!("Unable to delete user {}", request.user_id),
                ));
            }
        };
        if active_user.is_some_and(|user| user.id == request.user_id) {
            warn!("Invalid delete user request: {:?}", request.user_id);
            return Err(SdvStatus::with_message(
                SdvStatusCode::InvalidArgument,
                format!(
                    "Cannot delete user {} it is currently set as the active user",
                    request.user_id
                ),
            ));
        }

        if let Err(err) = self.persistence.users.delete(request.user_id) {
            match err.status_code() {
                //API should succeed when users don't exist
                SdvStatusCode::NotFound => {
                    info!("User {} doesn't exist", request.user_id);
                }
                _ => {
                    error!("Failed to delete user {}: {}", request.user_id, err);
                    return Err(SdvStatus::with_message(
                        SdvStatusCode::Internal,
                        format!("Unable to delete user {}", request.user_id),
                    ));
                }
            };
        }

        if let Err(err) = self.persistence.user_settings.delete_all(request.user_id) {
            error!("Failed to delete persisted settings for user {}: {}", request.user_id, err);
            return Err(SdvStatus::with_message(
                SdvStatusCode::Internal,
                format!("Unable to delete user settings {}", request.user_id),
            ));
        }

        info!("User {} was successfully deleted", request.user_id);
        Ok(DeleteUserResponse::new())
    }

    async fn FactoryReset(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        _request: &FactoryResetRequest,
    ) -> SdvResult<FactoryResetResponse> {
        self.persistence.user_settings.clear()?;
        self.persistence.users.clear()?;

        let services_to_reset: HashSet<String> = (self.persistence.setting_groups.list()?)
            .into_iter()
            .map(|group| group.service_fqin)
            .collect();

        for service_fqin in services_to_reset.iter() {
            let client = match self
                .rpc_client_factory
                .new_user_controllable_service_client(&ServiceFqin::from_str(service_fqin).unwrap())
                .await
            {
                Ok(client) => client,
                Err(err) => {
                    error!("Unable to factory reset {}: {}", service_fqin, err);
                    continue;
                }
            };
            if let Err(err) =
                client.FactoryReset(&user_controllable_service::FactoryResetRequest::new()).await
            {
                error!("Unable to factory reset {}: {}", service_fqin, err);
                continue;
            };
        }

        info!("Performed factory reset successfully");
        Ok(FactoryResetResponse::new())
    }

    async fn ListUsers(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        _request: &ListUsersRequest,
    ) -> SdvResult<ListUsersResponse> {
        info!("Got request to list users");
        let users = match self.persistence.users.list() {
            Ok(x) => x,
            Err(err) => {
                error!("Failed to list users: {}", err);
                return Err(err);
            }
        };

        let user_proto_list: Vec<UserProto> = users.into_iter().map_into().collect();
        info!("List users succeeded");

        Ok(ListUsersResponse { users: user_proto_list, ..Default::default() })
    }

    async fn GetUserSettings(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        request: &GetUserSettingsRequest,
    ) -> SdvResult<GetUserSettingsResponse> {
        info!("Got request to get_user_settings");

        if let Err(err) = self.persistence.users.get_user(request.user_id) {
            match err.status_code() {
                SdvStatusCode::NotFound => {
                    info!(
                        "Received a request to retrieve settings for user that doesn't exist: {}",
                        err
                    );
                }
                _ => {
                    error!("Failed to check if a user exists: {}", err);
                }
            }
            return Err(err);
        }

        let settings_groups = self.persistence.setting_groups.list()?;

        let mut setting_groups_protos: Vec<SettingsGroupProto> = Vec::new();
        for setting_group in settings_groups.into_iter() {
            let user_settings = retrieve_user_settings_including_defaults_for_group(
                &self.persistence.setting_definitions,
                &self.persistence.user_settings,
                request.user_id,
                setting_group.id,
            )?;

            setting_groups_protos.push(SettingsGroupProto {
                id: Some(SettingsGroupIdProto {
                    name: setting_group.name,
                    service_fqin: setting_group.service_fqin,
                    ..Default::default()
                })
                .into(),
                settings: user_settings.into_iter().map_into().collect(),
                ..Default::default()
            });
        }

        info!("get_user_settings succeeded");

        Ok(GetUserSettingsResponse { groups: setting_groups_protos, ..Default::default() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc_client_traits::MockUserControllableServiceClientTrait as MockUserControllableServiceClient;
    use crate::test_utils::*;
    use googletest::prelude::*;
    use protobuf::MessageField;
    use std::collections::HashMap;

    #[tokio::test]
    async fn create_user_succeeds() {
        let test_setup = TestSetup::new();
        let service = create_service_impl(&test_setup);
        let client_fqin = get_client_fqin();

        let user_proto =
            UserProto { id: 1, flags: UserFlags::DRIVER.value(), ..Default::default() };
        let request =
            CreateUserRequest { user: MessageField::some(user_proto), ..Default::default() };

        let response = service.CreateUser(client_fqin, &request).await;

        assert_that!(response, ok(anything()));
    }

    #[tokio::test]
    async fn create_user_fails_when_user_already_exists() {
        let test_setup = TestSetup::new();
        let service = create_service_impl(&test_setup);
        let client_fqin = get_client_fqin();

        let user_proto =
            UserProto { id: 1, flags: UserFlags::DRIVER.value(), ..Default::default() };
        let request =
            CreateUserRequest { user: MessageField::some(user_proto), ..Default::default() };

        // Create the user first
        service.CreateUser(client_fqin.clone(), &request).await.expect("Failed to create user");

        // Attempt to create the same user again
        let response = service.CreateUser(client_fqin.clone(), &request).await;

        // Assert that the response contains an error
        assert_that!(response, err(anything()));
        assert_eq!(response.unwrap_err().status_code(), SdvStatusCode::AlreadyExists);
    }

    #[tokio::test]
    async fn create_user_fails_for_invalid_flags() {
        let test_setup = TestSetup::new();
        let service = create_service_impl(&test_setup);
        let client_fqin = get_client_fqin();

        // Test with no flags set
        let user_proto = UserProto { id: 1, flags: 0, ..Default::default() };
        let request =
            CreateUserRequest { user: MessageField::some(user_proto), ..Default::default() };

        let response = service.CreateUser(client_fqin.clone(), &request).await;

        // Assert that the response contains an error
        assert_that!(response, err(anything()));
        assert_eq!(response.unwrap_err().status_code(), SdvStatusCode::InvalidArgument);

        // Test with invalid bits set
        let user_proto = UserProto {
            id: 1,
            flags: (UserFlags::EPHEMERAL.value() | UserFlags::DRIVER.value()) + 1,
            ..Default::default()
        };
        let request =
            CreateUserRequest { user: MessageField::some(user_proto), ..Default::default() };

        let response = service.CreateUser(client_fqin, &request).await;

        // Assert that the response contains an error
        assert_that!(response, err(anything()));
        assert_eq!(response.unwrap_err().status_code(), SdvStatusCode::InvalidArgument);
    }

    #[tokio::test]
    async fn create_user_fails_for_empty_request() {
        let test_setup = TestSetup::new();
        let service = create_service_impl(&test_setup);
        let client_fqin = get_client_fqin();

        let request = CreateUserRequest { user: MessageField::none(), ..Default::default() };

        let response = service.CreateUser(client_fqin, &request).await;

        // Assert that the response contains an error
        assert_that!(response, err(anything()));
        assert_eq!(response.unwrap_err().status_code(), SdvStatusCode::InvalidArgument);
    }

    #[tokio::test]
    async fn delete_user_succeeds() {
        let test_setup = TestSetup::new();
        let service = create_service_impl(&test_setup);
        let client_fqin = get_client_fqin();

        // Create a user
        let user_id = 1_i32;
        create_user_and_assert_success(&service, user_id, UserFlags::DRIVER.value()).await;

        // Delete the user
        let delete_user_request = DeleteUserRequest { user_id, ..Default::default() };
        let response = service.DeleteUser(client_fqin, &delete_user_request).await;

        // Assert that the response is ok
        assert_that!(response, ok(anything()));
    }

    #[tokio::test]
    async fn delete_user_succeeds_when_user_does_not_exist() {
        let test_setup = TestSetup::new();
        let service = create_service_impl(&test_setup);
        let client_fqin = get_client_fqin();

        // Attempt to delete a non-existent user
        let delete_user_request = DeleteUserRequest { user_id: 1, ..Default::default() };
        let response = service.DeleteUser(client_fqin, &delete_user_request).await;

        // Assert that the response contains an error
        assert_that!(response, ok(anything()));
    }

    #[tokio::test]
    async fn delete_user_fails_when_user_is_selected() {
        let test_setup = TestSetup::new();
        let service = create_service_impl(&test_setup);
        let client_fqin = get_client_fqin();

        let user_id = 1_i32;
        create_user_and_assert_success(&service, user_id, UserFlags::DRIVER.value()).await;

        select_user_and_assert_success(&service, user_id).await;

        let delete_user_request = DeleteUserRequest { user_id: 1, ..Default::default() };
        let response = service.DeleteUser(client_fqin, &delete_user_request).await;

        assert_that!(response, err(anything()));
        assert_eq!(response.unwrap_err().status_code(), SdvStatusCode::InvalidArgument);
    }

    #[tokio::test]
    async fn list_users_returns_all_users() {
        let test_setup = TestSetup::new();
        let service = create_service_impl(&test_setup);

        let users = [
            UserProto { id: 1, flags: UserFlags::DRIVER.value(), ..Default::default() },
            UserProto { id: 2, flags: UserFlags::DRIVER.value(), ..Default::default() },
            UserProto { id: 3, flags: UserFlags::DRIVER.value(), ..Default::default() },
        ];
        for user in users.iter() {
            let request =
                CreateUserRequest { user: MessageField::some(user.clone()), ..Default::default() };
            let _ = service
                .CreateUser(get_client_fqin(), &request)
                .await
                .expect("User Creation to succeed");
        }

        let mut response = service
            .ListUsers(get_client_fqin(), &ListUsersRequest::new())
            .await
            .expect("List Users to succeed");
        response.users.sort_by(|a, b| a.id.cmp(&b.id));

        assert_eq!(response.users.len(), 3);
        assert_eq!(response.users[0].id, 1);
        assert_eq!(response.users[1].id, 2);
        assert_eq!(response.users[2].id, 3);
    }

    #[tokio::test]
    async fn select_user_succeeds() {
        let mut test_setup = TestSetup::new();
        let (group, definitions) = test_setup.populate_persistence_with_settings_definitions();

        test_setup
            .mut_settings_change_propagator()
            .expect_queue_setting_change()
            .withf(move |fqin, group_name, settings| {
                fqin.to_string() == group.service_fqin
                    && group_name == &group.name
                    && settings.len() == definitions.len()
            })
            .returning(|_, _, _| ());

        let service = create_service_impl(&test_setup);
        create_user_and_assert_success(&service, 1, UserFlags::DRIVER.value()).await;
        select_user_and_assert_success(&service, 1).await;
    }

    #[tokio::test]
    async fn select_user_fails_when_user_does_not_exist() {
        let test_setup = TestSetup::new();
        let service = create_service_impl(&test_setup);

        let request = SelectUserRequest { user_id: 1, ..Default::default() };
        let response = service.SelectUser(get_client_fqin(), &request).await;

        assert_that!(response, err(anything()));
        assert_eq!(response.unwrap_err().status_code(), SdvStatusCode::NotFound);
    }

    #[tokio::test]
    async fn select_user_deletes_ephemeral_user_on_switch() {
        let mut test_setup = TestSetup::new();
        let _ = test_setup.populate_persistence_with_settings_definitions();

        test_setup
            .mut_settings_change_propagator()
            .expect_queue_setting_change()
            .times(2)
            .returning(|_, _, _| ());

        let service = create_service_impl(&test_setup);
        create_user_and_assert_success(
            &service,
            1,
            UserFlags::DRIVER.value() | UserFlags::EPHEMERAL.value(),
        )
        .await;
        select_user_and_assert_success(&service, 1).await;

        // Switch to a different user to check if the former user will be deleted
        create_user_and_assert_success(&service, 2, UserFlags::DRIVER.value()).await;
        select_user_and_assert_success(&service, 2).await;

        let users = service.persistence.users.list().unwrap();
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].id, 2);
    }

    #[tokio::test]
    async fn get_user_settings_returns_error_for_non_existent_user() {
        let test_setup = TestSetup::new();
        let service = create_service_impl(&test_setup);
        let non_existent_user_id = 999;

        let request =
            GetUserSettingsRequest { user_id: non_existent_user_id, ..Default::default() };
        let response = service.GetUserSettings(get_client_fqin(), &request).await;

        assert_that!(response, err(anything()));
        assert_eq!(response.unwrap_err().status_code(), SdvStatusCode::NotFound);
    }

    #[tokio::test]
    async fn get_user_settings_returns_correct_settings_for_existing_user() {
        let test_setup = TestSetup::new();
        test_setup.populate_persistence_with_settings_definitions();
        let (group, definitions) = test_setup.populate_persistence_with_settings_definitions();
        let service = create_service_impl(&test_setup);
        let user_id = 1;

        create_user_and_assert_success(&service, user_id, UserFlags::DRIVER.value()).await;

        let request = GetUserSettingsRequest { user_id, ..Default::default() };
        let response = service
            .GetUserSettings(get_client_fqin(), &request)
            .await
            .expect("Get User Settings to succeed");

        assert_eq!(response.groups.len(), 1);
        assert_eq!(response.groups[0].id.as_ref().unwrap().name, group.name);
        assert_eq!(response.groups[0].settings.len(), 2);

        let mut settings_map = HashMap::new();
        for setting in response.groups[0].settings.iter() {
            settings_map.insert(setting.key.clone(), setting);
        }

        for definition in definitions {
            assert_eq!(
                settings_map.get(&definition.name).unwrap().value,
                Some(definition.default_value.clone().into())
            );
        }
    }

    #[tokio::test]
    async fn factory_reset_succeeds() {
        let mut test_setup = TestSetup::new();
        test_setup.populate_persistence_with_settings_definitions();

        // Ensure that the remote service gets a FactoryReset request
        test_setup
            .mut_rpc_client_factory()
            .expect_new_user_controllable_service_client()
            .returning(move |_| {
                let mut mock_client = Box::new(MockUserControllableServiceClient::new());
                mock_client
                    .expect_FactoryReset()
                    .returning(|_| Ok(user_controllable_service::FactoryResetResponse::new()));
                Ok(mock_client)
            });
        let service = create_service_impl(&test_setup);

        create_user_and_assert_success(&service, 1, UserFlags::DRIVER.value()).await;

        let response = service.FactoryReset(get_client_fqin(), &FactoryResetRequest::new()).await;
        assert_that!(response, ok(anything()));

        // Ensure all user accounts have been deleted
        let response = service
            .ListUsers(get_client_fqin(), &ListUsersRequest::new())
            .await
            .expect("List users to succeed");
        assert_eq!(response.users.len(), 0);
    }
}
