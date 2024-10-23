// Copyright 2024 Google LLC
//! Sample user preferences implementation

use crate::persistence::setting::Setting;
use crate::persistence::setting_definition::SettingDefinition;
use crate::persistence::settings_group_repository::SettingsGroup;
use crate::{
    persistence::Persistence, settings_change_propagator_impl::SettingsChangePropagatorImpl,
};
use async_trait::async_trait;
use itertools::Itertools;
use log::{error, info, warn};
use sdv_comms::id::ServiceFqin;
use sdv_status_rs::{SdvResult, SdvStatus, SdvStatusCode};
use sdv_user_preferences_user_preferences_management_service_rpc::Interface as ManagementService;
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
}

impl ManagementServiceImpl {
    pub fn new(
        persistence: Arc<Persistence>,
        settings_change_propagator: Arc<SettingsChangePropagatorImpl>,
    ) -> Self {
        Self { persistence, settings_change_propagator }
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
        _caller_id: sdv_comms::id::ServiceFqin,
        _request: &SubscribeToSettingsChangeAndGetSettingsRequest,
    ) -> sdv_status_rs::SdvResult<SubscribeToSettingsChangeAndGetSettingsResponse> {
        Err(sdv_status_rs::SdvStatus::new(sdv_status_rs::SdvStatusCode::Unimplemented))
    }

    async fn UnsubscribeFromSettingsChange(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        _request: &UnsubscribeToSettingsChangeRequest,
    ) -> sdv_status_rs::SdvResult<UnsubscribeToSettingsChangeResponse> {
        Err(sdv_status_rs::SdvStatus::new(sdv_status_rs::SdvStatusCode::Unimplemented))
    }
}
