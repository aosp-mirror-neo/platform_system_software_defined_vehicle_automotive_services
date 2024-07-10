// Copyright 2023 Google LLC

use itertools::Itertools;
use std::collections::HashSet;

use sdvgenerated::sdv_user_preferences_user_preferences_admin_service_interface::UserPreferencesAdminService;
use sdvgenerated::sdv_user_preferences_user_preferences_management_service_interface::UserPreferencesManagementService;
use sdvgenerated::sdv_user_preferences_user_preferences_registry_service_interface::UserPreferencesRegistryService;

use user_preferences_api::user_preferences_admin_service::*;
use user_preferences_api::user_preferences_management_service::*;
use user_preferences_api::user_preferences_registry_service::{
    RegisterSettingsRequest, RegisterSettingsResponse, UpdateSettingsRequest,
    UpdateSettingsResponse,
};

use sdvmiddleware_rpc::RpcResult;

pub struct UserPreferencesServiceImpl {}

impl UserPreferencesServiceImpl {
    pub fn new() -> Self {
        UserPreferencesServiceImpl {}
    }
}

impl UserPreferencesAdminService for UserPreferencesServiceImpl {
    fn create_user(&self, _request: CreateUserRequest) -> RpcResult<CreateUserResponse> {
        RpcResult::Err(sdvmiddleware_rpc::error::RpcError::Other)
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
