// Copyright 2024 Google LLC
//! Sample user preferences implementation

use crate::persistence::setting::Setting;
use crate::persistence::setting_definition::SettingDefinition;
use crate::persistence::settings_group_repository::SettingsGroup;
use crate::settings_retrieval_utils::{
    retrieve_user_settings_including_defaults_for_group,
    retrieve_vehicle_settings_including_defaults_for_group,
};
use crate::subscriptions_manager_impl::SubscriptionsManagerImpl;
use crate::{
    persistence::Persistence, settings_change_propagator_impl::SettingsChangePropagatorImpl,
};
use async_trait::async_trait;
use com_sdv_google_user_preferences_user_preferences_management_service_rpc::Interface as ManagementService;
use itertools::Itertools;
use log::{error, info, warn};
use sdv_comms::id::ServiceFqin;
use sdv_status_rs::{SdvResult, SdvStatus, SdvStatusCode};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use user_preferences_api::setting::Setting as SettingProto;
use user_preferences_api::user_preferences_management_service::{
    request_settings_change_request::ChangePersistencePolicy, *,
};

pub struct ManagementServiceImpl {
    persistence: Arc<Persistence>,
    settings_change_propagator: Arc<SettingsChangePropagatorImpl>,
    subscriptions_manager: Arc<SubscriptionsManagerImpl>,
}

impl ManagementServiceImpl {
    pub fn new(
        persistence: Arc<Persistence>,
        settings_change_propagator: Arc<SettingsChangePropagatorImpl>,
        subscriptions_manager: Arc<SubscriptionsManagerImpl>,
    ) -> Self {
        Self { persistence, settings_change_propagator, subscriptions_manager }
    }

    fn are_valid_settings(
        &self,
        settings_group: &SettingsGroup,
        settings: &[SettingProto],
    ) -> SdvResult<()> {
        let settings_definitions: Vec<SettingDefinition> =
            self.persistence.setting_definitions.get(settings_group.id).map_err(|err| {
                info!("Failed to retrieve settings definition: {}", err);
                SdvStatus::new(SdvStatusCode::Internal)
            })?;
        let settings_definitions: HashMap<String, SettingDefinition> = settings_definitions
            .into_iter()
            .map(|setting_definition| (setting_definition.name.clone(), setting_definition))
            .collect::<HashMap<_, _>>();

        for setting in settings {
            let setting_definition = settings_definitions.get(&setting.key).ok_or_else(|| {
                SdvStatus::with_message(
                    SdvStatusCode::InvalidArgument,
                    format!("Setting {} not found in settings group", setting.key),
                )
            })?;

            setting_definition.is_valid_value(&setting.value.clone().unwrap().into()).map_err(
                |err| SdvStatus::with_message(SdvStatusCode::InvalidArgument, format!("{}", err)),
            )?;
        }

        Ok(())
    }
}

#[async_trait]
impl ManagementService for ManagementServiceImpl {
    async fn RequestSettingsChange(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        request: &RequestSettingsChangeRequest,
    ) -> sdv_status_rs::SdvResult<RequestSettingsChangeResponse> {
        let settings_group_id = match request.settings_group_id.clone().into_option() {
            Some(x) => x,
            None => {
                warn!("Change settings request contains nil settings group id");
                return Err(SdvStatus::new(SdvStatusCode::InvalidArgument));
            }
        };
        let setting_group: SettingsGroup = match self
            .persistence
            .setting_groups
            .get(settings_group_id.name.as_str(), settings_group_id.service_fqin.as_str())
        {
            Ok(x) => x,
            Err(err) => {
                error!("Failed to request setting change: {:?}", err);
                return Err(SdvStatus::new(SdvStatusCode::Internal));
            }
        };
        if let Err(err) = self.are_valid_settings(&setting_group, &request.settings) {
            warn!("Received request containing invalid settings: {:?}", err);
            return Err(err);
        }

        if request.change_persistence_policy == ChangePersistencePolicy::PERSISTENT_CHANGE.into() {
            let current_user = match self.persistence.users.get_active_user() {
                Ok(user) => user,
                Err(err) => {
                    info!("Unable to find active user: {err:?}");
                    return Err(SdvStatus::with_message(
                        SdvStatusCode::Internal,
                        "Failed to perform settings change",
                    ));
                }
            };
            if let Some(current_user) = current_user {
                let settings: Vec<Setting> = request.settings.iter().cloned().map_into().collect();
                self.persistence.user_settings.store_all(
                    current_user.id,
                    setting_group.id,
                    &settings,
                )?;
            } else {
                return Err(SdvStatus::with_message(
                    SdvStatusCode::Internal,
                    "Cannot perform persistent changes when no user is selected",
                ));
            }
        }

        self.settings_change_propagator
            .queue_setting_change(
                ServiceFqin::from_str(&setting_group.service_fqin).unwrap(),
                setting_group.name,
                request.settings.iter().cloned().map_into().collect_vec(),
            )
            .await;

        Ok(RequestSettingsChangeResponse::new())
    }

    async fn SubscribeToSettingsChangeAndGetSettings(
        &self,
        caller_id: sdv_comms::id::ServiceFqin,
        request: &SubscribeToSettingsChangeAndGetSettingsRequest,
    ) -> sdv_status_rs::SdvResult<SubscribeToSettingsChangeAndGetSettingsResponse> {
        let settings_group_id = request.settings_group_id.as_ref().unwrap();
        let settings_group = match self
            .persistence
            .setting_groups
            .get(settings_group_id.name.as_str(), settings_group_id.service_fqin.as_str())
        {
            Ok(x) => x,
            Err(err) => {
                error!(
                    "SubscribeToSettingsChangeAndGetSettings failed to retrieve settings group: {}",
                    err
                );
                return Err(err);
            }
        };

        let active_user = self.persistence.users.get_active_user()?;

        let active_settings = retrieve_vehicle_settings_including_defaults_for_group(
            &self.persistence.setting_definitions,
            &self.persistence.vehicle_settings,
            settings_group.id,
        )?;
        let persisted_settings: Vec<SettingProto> = match active_user {
            Some(user) => {
                match retrieve_user_settings_including_defaults_for_group(
                    &self.persistence.setting_definitions,
                    &self.persistence.user_settings,
                    user.id,
                    settings_group.id,
                ) {
                    Ok(x) => x.into_iter().map_into().collect_vec(),
                    Err(err) => {
                        error!("Failed to retrieve persisted settings: {}", err);
                        return Err(err);
                    }
                }
            }
            None => Vec::new(),
        };

        let pending_settings: Vec<SettingProto> = self
            .settings_change_propagator
            .get_pending_setting_changes(
                &ServiceFqin::from_str(&settings_group_id.service_fqin).unwrap(),
                &settings_group_id.name,
            )
            .await
            .into_iter()
            .map_into()
            .collect();

        self.subscriptions_manager.register_subscriber(settings_group_id.clone(), caller_id).await;

        Ok(SubscribeToSettingsChangeAndGetSettingsResponse {
            active_settings,
            pending_settings,
            persisted_settings,
            version: settings_group.version,
            ..Default::default()
        })
    }

    async fn UnsubscribeFromSettingsChange(
        &self,
        caller_id: sdv_comms::id::ServiceFqin,
        request: &UnsubscribeToSettingsChangeRequest,
    ) -> sdv_status_rs::SdvResult<UnsubscribeToSettingsChangeResponse> {
        let settings_group_id = request.settings_group_id.as_ref().unwrap();

        self.subscriptions_manager
            .unregister_subscriber(settings_group_id.clone(), caller_id)
            .await;

        Ok(UnsubscribeToSettingsChangeResponse::new())
    }
}
