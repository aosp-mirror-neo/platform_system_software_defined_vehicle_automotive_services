// Copyright 2024 Google LLC
//! Sample user preferences implementation
//!
use std::sync::Arc;

use crate::persistence::setting::Setting;
use crate::persistence::setting_definition::SettingDefinition;
use crate::persistence::settings_group_repository::SettingsGroup;
use crate::persistence::Persistence;
use crate::settings_change_propagator_impl::SettingsChangePropagatorImpl;

use async_trait::async_trait;
use itertools::Itertools;
use log::{error, info, warn};
use sdv_status_rs::{SdvStatus, SdvStatusCode};
use sdv_user_preferences_user_preferences_registry_service_rpc::Interface as RegistryService;
use std::collections::HashSet;
use user_preferences_api::setting::SettingDefinition as SettingDefinitionProto;
use user_preferences_api::user_preferences_registry_service::*;

pub struct RegistryServiceImpl {
    persistence: Arc<Persistence>,
    settings_change_propagator: Arc<SettingsChangePropagatorImpl>,
}

impl RegistryServiceImpl {
    pub fn new(
        persistence: Arc<Persistence>,
        settings_change_propagator: Arc<SettingsChangePropagatorImpl>,
    ) -> Self {
        Self { persistence, settings_change_propagator }
    }
}

#[async_trait]
impl RegistryService for RegistryServiceImpl {
    async fn RegisterSettings(
        &self,
        caller_id: sdv_comms::id::ServiceFqin,
        request: &RegisterSettingsRequest,
    ) -> sdv_status_rs::SdvResult<RegisterSettingsResponse> {
        let group_id = match self.persistence.setting_groups.store(&SettingsGroup {
            id: 0,
            name: request.group_name.clone(),
            service_fqin: caller_id.to_string(),
        }) {
            Ok(x) => x,
            Err(err) => {
                error!("Failed to register settings group: {:?}", err);
                return Err(SdvStatus::with_message(
                    SdvStatusCode::Internal,
                    "Unexpected error, unable to register settings",
                ));
            }
        };

        let definition = request
            .settings_definitions
            .iter()
            .cloned()
            .map(|definition_proto: SettingDefinitionProto| {
                let setting_with_constraints = definition_proto.setting_with_constraints.unwrap();
                let setting = setting_with_constraints.setting.unwrap();
                SettingDefinition {
                    name: setting.key,
                    kind: definition_proto.kind.value(),
                    default_value: setting.value.unwrap().into(),
                    constraint: setting_with_constraints.constraints.map(|x| x.into()),
                }
            })
            .collect_vec();

        match self.persistence.setting_definitions.store(group_id, &definition) {
            Ok(_) => {
                info!(
                    "Registered setting group {}/{}\n{:?}",
                    caller_id.to_string(),
                    request.group_name,
                    definition
                );
                Ok(RegisterSettingsResponse::new())
            }
            Err(err) => {
                error!("Failed to register settings group: {:?}", err);
                return Err(SdvStatus::with_message(
                    SdvStatusCode::Internal,
                    "Unexpected error, unable to register settings",
                ));
            }
        }
    }

    async fn UpdateSettings(
        &self,
        caller_id: sdv_comms::id::ServiceFqin,
        request: &UpdateSettingsRequest,
    ) -> sdv_status_rs::SdvResult<UpdateSettingsResponse> {
        let settings_group = match self
            .persistence
            .setting_groups
            .get(request.group_name.as_str(), &caller_id.to_string())
        {
            Ok(x) => x,
            Err(err) => {
                if err.status_code() == SdvStatusCode::NotFound {
                    warn!(
                        "Settings group {}/{} does not exist",
                        request.group_name,
                        caller_id.to_string(),
                    );
                } else {
                    error!("Unable to update setting: {:?}", err);
                }
                return Err(err);
            }
        };

        let settings: Vec<Setting> = request.settings.iter().cloned().map_into().collect_vec();
        info!("Updating vehicle settings in group: {}\n{:?}", settings_group, settings);

        if let Err(err) = self.persistence.vehicle_settings.store_all(settings_group.id, settings) {
            error!("Encountered failure when updating vehicle settings repository: {}", err);
            return Err(SdvStatus::new(SdvStatusCode::Internal));
        }

        let updated_settings_name: HashSet<String> =
            request.settings.iter().map(|setting| setting.key.clone()).collect();
        self.settings_change_propagator
            .notify_subscribers_of_current_state(
                &caller_id,
                &settings_group.name,
                &updated_settings_name,
            )
            .await;

        return Ok(UpdateSettingsResponse::new());
    }
}
