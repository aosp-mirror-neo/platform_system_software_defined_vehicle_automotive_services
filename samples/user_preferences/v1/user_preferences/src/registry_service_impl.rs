// Copyright 2024 Google LLC
//! Sample user preferences implementation
//!
use std::sync::Arc;

use crate::persistence::setting_definition::SettingDefinition;
use crate::persistence::settings_group_repository::SettingsGroup;
use crate::persistence::Persistence;
use async_trait::async_trait;
use itertools::Itertools;
use log::{error, info};
use sdv_status_rs::{SdvStatus, SdvStatusCode};
use sdv_user_preferences_user_preferences_registry_service_rpc::Interface as RegistryService;
use user_preferences_api::setting::SettingDefinition as SettingDefinitionProto;
use user_preferences_api::user_preferences_registry_service::*;

pub struct RegistryServiceImpl {
    persistence: Arc<Persistence>,
}

impl RegistryServiceImpl {
    pub fn new(persistence: Arc<Persistence>) -> Self {
        Self { persistence }
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
        _caller_id: sdv_comms::id::ServiceFqin,
        _request: &UpdateSettingsRequest,
    ) -> sdv_status_rs::SdvResult<UpdateSettingsResponse> {
        Err(sdv_status_rs::SdvStatus::new(sdv_status_rs::SdvStatusCode::Unimplemented))
    }
}
