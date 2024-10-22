// Copyright 2024 Google LLC
//! Sample user preferences implementation
use sdv_user_preferences_user_preferences_admin_service_rpc::Interface as AdminService;

use crate::persistence::user_repository::User;
use crate::persistence::Persistence;
use async_trait::async_trait;
use log::{error, info, warn};
use protobuf::Enum;
use sdv_status_rs::{SdvResult, SdvStatus, SdvStatusCode};
use std::sync::Arc;
use user_preferences_api::user::user::UserFlags;
use user_preferences_api::user_preferences_admin_service::*;

pub struct AdminServiceImpl {
    persistence: Arc<Persistence>,
}

impl AdminServiceImpl {
    pub fn new(persistence: Arc<Persistence>) -> Self {
        Self { persistence }
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
        _caller_id: sdv_comms::id::ServiceFqin,
        _request: &SelectUserRequest,
    ) -> SdvResult<SelectUserResponse> {
        Err(sdv_status_rs::SdvStatus::new(sdv_status_rs::SdvStatusCode::Unimplemented))
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
        Err(sdv_status_rs::SdvStatus::new(sdv_status_rs::SdvStatusCode::Unimplemented))
    }

    async fn ListUsers(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        _request: &ListUsersRequest,
    ) -> SdvResult<ListUsersResponse> {
        Err(sdv_status_rs::SdvStatus::new(sdv_status_rs::SdvStatusCode::Unimplemented))
    }

    async fn GetUserSettings(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        _request: &GetUserSettingsRequest,
    ) -> SdvResult<GetUserSettingsResponse> {
        Err(sdv_status_rs::SdvStatus::new(sdv_status_rs::SdvStatusCode::Unimplemented))
    }
}
