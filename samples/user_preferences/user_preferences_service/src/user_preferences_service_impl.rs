// Copyright 2023 Google LLC

use sdvgenerated::sdv_user_preferences_user_preferences_admin_service_interface::UserPreferencesAdminService;
use sdvgenerated::sdv_user_preferences_user_preferences_management_service_interface::UserPreferencesManagementService;
use sdvgenerated::sdv_user_preferences_user_preferences_registry_service_interface::UserPreferencesRegistryService;

use user_preferences_api::error::{Error, ResponseCode};
use user_preferences_api::user_preferences_admin_service::*;
use user_preferences_api::user_preferences_management_service::*;
use user_preferences_api::user_preferences_registry_service::{
    RegisterSettingsRequest, RegisterSettingsResponse, UpdateSettingsRequest,
    UpdateSettingsResponse,
};

use crate::macros::error_response;
use crate::persistence::user_repository::{User, UserRepository};
use log::{error, info};

use protobuf::{EnumOrUnknown, MessageField};
use sdvmiddleware_rpc::RpcResult;

pub struct UserPreferencesServiceImpl {
    user_repository: UserRepository,
}

impl UserPreferencesServiceImpl {
    pub fn new(user_repository: UserRepository) -> Self {
        UserPreferencesServiceImpl { user_repository }
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
                crate::persistence::user_repository::UserRepositoryError::AlreadyExists => {
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
        _request: RegisterSettingsRequest,
    ) -> RpcResult<RegisterSettingsResponse> {
        RpcResult::Err(sdvmiddleware_rpc::error::RpcError::Other)
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

    fn create_user_preferences_impl() -> UserPreferencesServiceImpl {
        let connection = Arc::new(Mutex::new(
            Connection::open_in_memory().expect("Failed to create connection"),
        ));
        let user_repository = UserRepository::new(connection.clone())
            .expect("Failed to create an instance of user repository");

        UserPreferencesServiceImpl::new(user_repository)
    }
}
