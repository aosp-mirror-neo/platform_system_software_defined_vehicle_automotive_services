// Copyright 2024 Google LLC
//! Sample user preferences implementation
use async_trait::async_trait;
use com_sdv_google_user_preferences_user_controllable_user_controllable_service_rpc::Client as UserControllableServiceClient;
use com_sdv_google_user_preferences_view_change_notifier_rpc::Client as ChangeNotifierClient;
use mockall::automock;
use sdv_status_rs::SdvResult;

#[automock]
#[async_trait]
pub trait UserControllableServiceClientTrait: Send + Sync {
    #[allow(non_snake_case)]
    async fn RequestSettingsChange(
        &self,
        request: &user_preferences_api::user_controllable_service::RequestSettingsChangeRequest,
    ) -> SdvResult<user_preferences_api::user_controllable_service::RequestSettingsChangeResponse>;
    #[allow(non_snake_case)]
    async fn FactoryReset(
        &self,
        request: &user_preferences_api::user_controllable_service::FactoryResetRequest,
    ) -> SdvResult<user_preferences_api::user_controllable_service::FactoryResetResponse>;
}

#[async_trait]
impl UserControllableServiceClientTrait for UserControllableServiceClient {
    async fn RequestSettingsChange(
        &self,
        request: &user_preferences_api::user_controllable_service::RequestSettingsChangeRequest,
    ) -> SdvResult<user_preferences_api::user_controllable_service::RequestSettingsChangeResponse>
    {
        UserControllableServiceClient::RequestSettingsChange(self, request).await
    }

    async fn FactoryReset(
        &self,
        request: &user_preferences_api::user_controllable_service::FactoryResetRequest,
    ) -> SdvResult<user_preferences_api::user_controllable_service::FactoryResetResponse> {
        UserControllableServiceClient::FactoryReset(self, request).await
    }
}

#[automock]
#[async_trait]
pub trait ChangeNotifierClientTrait: Send + Sync {
    #[allow(non_snake_case)]
    async fn OnSettingsChange(
        &self,
        request: &user_preferences_api::change_notifier::OnSettingsChangeRequest,
    ) -> SdvResult<user_preferences_api::change_notifier::OnSettingsChangeResponse>;
}

#[async_trait]
impl ChangeNotifierClientTrait for ChangeNotifierClient {
    async fn OnSettingsChange(
        &self,
        request: &user_preferences_api::change_notifier::OnSettingsChangeRequest,
    ) -> SdvResult<user_preferences_api::change_notifier::OnSettingsChangeResponse> {
        ChangeNotifierClient::OnSettingsChange(self, request).await
    }
}
