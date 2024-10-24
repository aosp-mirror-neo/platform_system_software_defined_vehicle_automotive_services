// Copyright 2024 Google LLC
#![allow(missing_docs)]

use async_trait::async_trait;
use com_sdv_google_user_preferences_user_controllable_user_controllable_service_rpc::Interface as UserControllableService;
use com_sdv_google_user_preferences_user_preferences_registry_service_rpc::Client as RegistryServiceClient;
use itertools::Itertools;
use protobuf::MessageField;
use sdv_status_rs::SdvStatus;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use user_preferences_api::{
    setting::{SettingDefinition, SettingWithConstraints},
    user_controllable_service::{FactoryResetResponse, RequestSettingsChangeResponse},
    user_preferences_registry_service::{RegisterSettingsRequest, UpdateSettingsRequest},
};

// Definition of the settings group a service provides to users.
#[derive(Clone)]
pub struct SettingGroupDefinition {
    // Name of the settings group.
    pub name: String,
    // Version of the schema of this group.
    pub version: String,
    // The definition of the settings within this group.
    pub setting_definitions: Vec<SettingDefinition>,
}

#[derive(Clone)]
struct SettingGroup {
    name: String,
    setting_with_constraints: HashMap<String, SettingWithConstraints>,
}

pub struct SimpleUserControllableServiceImpl {
    setting_groups_definitions: Vec<SettingGroupDefinition>,
    setting_groups: Mutex<HashMap<String, SettingGroup>>,
    client: Arc<RegistryServiceClient>,
}

impl SimpleUserControllableServiceImpl {
    pub async fn new(
        setting_groups_definitions: Vec<SettingGroupDefinition>,
        client: Arc<RegistryServiceClient>,
    ) -> Self {
        let service_impl =
            Self { setting_groups_definitions, setting_groups: Mutex::new(HashMap::new()), client };

        service_impl.init_setting_groups().await;
        service_impl.register_settings().await;

        service_impl
    }

    async fn init_setting_groups(&self) {
        let mut setting_groups = self.setting_groups.lock().await;

        for group in self.setting_groups_definitions.iter().cloned() {
            setting_groups.insert(
                group.name.clone(),
                SettingGroup {
                    name: group.name,
                    setting_with_constraints: group
                        .setting_definitions
                        .into_iter()
                        .map(|setting_definition| {
                            let setting_and_constraint =
                                setting_definition.setting_with_constraints.into_option().unwrap();
                            (
                                setting_and_constraint.setting.as_ref().unwrap().key.clone(),
                                setting_and_constraint,
                            )
                        })
                        .collect(),
                },
            );
        }
    }

    async fn register_settings(&self) {
        for group in self.setting_groups_definitions.iter().cloned() {
            println!(
                "Registering settings group with name: {},\nVersion: {}",
                group.name, group.version,
            );

            self.client
                .RegisterSettings(&RegisterSettingsRequest {
                    group_name: group.name,
                    version: group.version,
                    settings_definitions: group.setting_definitions,
                    ..Default::default()
                })
                .await
                .expect("Failed to register settings");
        }
    }
}

#[async_trait]
impl UserControllableService for SimpleUserControllableServiceImpl {
    async fn RequestSettingsChange(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        request: &user_preferences_api::user_controllable_service::RequestSettingsChangeRequest,
    ) -> sdv_status_rs::SdvResult<
        user_preferences_api::user_controllable_service::RequestSettingsChangeResponse,
    > {
        let mut setting_groups = self.setting_groups.lock().await;

        let setting_group = match setting_groups.get_mut(&request.group_name) {
            Some(x) => x,
            None => {
                return Err(SdvStatus::with_message(
                    sdv_status_rs::SdvStatusCode::NotFound,
                    format!("Settings group {} not found", request.group_name),
                ));
            }
        };

        println!("Adjusted the following settings: ");
        for new_setting in request.settings.iter() {
            let persisted_setting =
                setting_group.setting_with_constraints.get_mut(&new_setting.key);
            persisted_setting.unwrap().setting = MessageField::some(new_setting.clone());

            println!("Key {:<15} Value {:<10?}", new_setting.key, new_setting.value)
        }

        // Respond that all settings were applied as requested
        Ok(RequestSettingsChangeResponse {
            settings: request.settings.clone(),
            ..Default::default()
        })
    }

    async fn FactoryReset(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        _request: &user_preferences_api::user_controllable_service::FactoryResetRequest,
    ) -> sdv_status_rs::SdvResult<
        user_preferences_api::user_controllable_service::FactoryResetResponse,
    > {
        self.init_setting_groups().await;

        let setting_groups = self.setting_groups.lock().await;
        for setting_group in setting_groups.values() {
            let settings = setting_group
                .setting_with_constraints
                .values()
                .map(|setting| setting.setting.clone().unwrap())
                .collect_vec();

            self.client
                .UpdateSettings(&UpdateSettingsRequest {
                    group_name: setting_group.name.clone(),
                    settings,
                    ..Default::default()
                })
                .await
                .expect("Unexpected failure to update settings");

            println!("\nPerformed factory reset");
        }
        Ok(FactoryResetResponse::new())
    }
}
