// Copyright 2024 Google LLC
//! Sample user preferences implementation
//!
use std::sync::Arc;

use crate::persistence::setting::Setting;
use crate::persistence::setting_definition::SettingDefinition;
use crate::persistence::settings_group_repository::SettingsGroup;
use crate::persistence::Persistence;
use crate::settings_change_propagator_impl::SettingsChangePropagator;

use async_trait::async_trait;
use com_sdv_google_user_preferences_user_preferences_registry_service_rpc::Interface as RegistryService;
use itertools::Itertools;
use log::{error, info, warn};
use sdv_status_rs::{SdvResult, SdvStatus, SdvStatusCode};
use std::collections::HashSet;
use user_preferences_api::setting::SettingDefinition as SettingDefinitionProto;
use user_preferences_api::user_preferences_registry_service::*;

pub struct RegistryServiceImpl {
    persistence: Arc<Persistence>,
    settings_change_propagator: Arc<dyn SettingsChangePropagator>,
}

impl RegistryServiceImpl {
    pub fn new(
        persistence: Arc<Persistence>,
        settings_change_propagator: Arc<dyn SettingsChangePropagator>,
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
    ) -> SdvResult<RegisterSettingsResponse> {
        let group_id = match self.persistence.setting_groups.store(&SettingsGroup {
            id: 0,
            name: request.group_name.clone(),
            service_fqin: caller_id.to_string(),
            version: request.version.clone(),
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
                let setting_with_constraints =
                    definition_proto.setting_with_constraints.into_option()?;
                let setting = setting_with_constraints.setting.into_option()?;
                Some(SettingDefinition {
                    name: setting.key,
                    kind: definition_proto.kind.value(),
                    default_value: setting.value?.into(),
                    constraint: setting_with_constraints.constraints.map(|x| x.into()),
                })
            })
            .collect::<Option<Vec<_>>>()
            .ok_or(SdvStatus::new(SdvStatusCode::NotFound))?;

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
    ) -> SdvResult<UpdateSettingsResponse> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::*;
    use googletest::assert_that;
    use googletest::prelude::*;
    use user_preferences_api::setting::setting::Value;
    use user_preferences_api::setting::Setting as SettingProto;
    use user_preferences_api::setting::SettingKind;
    use user_preferences_api::setting::SettingWithConstraints;

    #[tokio::test]
    async fn register_settings_succeeds() {
        let test_setup = TestSetup::new();
        let service = RegistryServiceImpl::new(
            test_setup.persistence.clone(),
            test_setup.settings_change_propagator.clone(),
        );

        let request = RegisterSettingsRequest {
            group_name: "display".to_string(),
            version: "1.0".to_string(),
            settings_definitions: vec![
                SettingDefinitionProto {
                    kind: SettingKind::PER_USER.into(),
                    setting_with_constraints: Some(SettingWithConstraints {
                        setting: Some(SettingProto {
                            key: "brightness".to_string(),
                            value: Some(Value::Int32(50)),
                            ..Default::default()
                        })
                        .into(),
                        ..Default::default()
                    })
                    .into(),
                    ..Default::default()
                },
                SettingDefinitionProto {
                    kind: SettingKind::PER_USER.into(),
                    setting_with_constraints: Some(SettingWithConstraints {
                        setting: Some(SettingProto {
                            key: "contrast".to_string(),
                            value: Some(Value::Int32(60)),
                            ..Default::default()
                        })
                        .into(),
                        ..Default::default()
                    })
                    .into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let response =
            service.RegisterSettings(get_user_controllable_service_fqin(), &request).await;
        assert_that!(response, ok(anything()));

        let settings_group = test_setup
            .persistence
            .setting_groups
            .get("display", &get_user_controllable_service_fqin().to_string())
            .expect("Failed to get settings group");
        assert_eq!(settings_group.name, "display");
        assert_eq!(settings_group.version, "1.0");

        let settings_definitions = test_setup
            .persistence
            .setting_definitions
            .get(settings_group.id)
            .expect("Failed to get settings definitions");
        assert_eq!(settings_definitions.len(), 2);
        assert_eq!(settings_definitions[0].name, "brightness");
        assert_eq!(settings_definitions[1].name, "contrast");
    }

    #[tokio::test]
    async fn update_settings_succeeds() {
        let mut test_setup = TestSetup::new();

        test_setup
            .mut_settings_change_propagator()
            .expect_notify_subscribers_of_current_state()
            .withf(move |fqin, group_name, updated_settings_names| {
                fqin == &get_user_controllable_service_fqin()
                    && group_name == "display"
                    && updated_settings_names.contains("brightness")
                    && updated_settings_names.contains("contrast")
            })
            .returning(|_, _, _| ());

        let service = RegistryServiceImpl::new(
            test_setup.persistence.clone(),
            test_setup.settings_change_propagator.clone(),
        );

        // Register a settings group
        let _ = service
            .RegisterSettings(
                get_user_controllable_service_fqin(),
                &RegisterSettingsRequest {
                    group_name: "display".to_string(),
                    version: "1.0".to_string(),
                    settings_definitions: vec![
                        SettingDefinitionProto {
                            kind: SettingKind::PER_USER.into(),
                            setting_with_constraints: Some(SettingWithConstraints {
                                setting: Some(SettingProto {
                                    key: "brightness".to_string(),
                                    value: Some(Value::Int32(50)),
                                    ..Default::default()
                                })
                                .into(),
                                ..Default::default()
                            })
                            .into(),
                            ..Default::default()
                        },
                        SettingDefinitionProto {
                            kind: SettingKind::PER_USER.into(),
                            setting_with_constraints: Some(SettingWithConstraints {
                                setting: Some(SettingProto {
                                    key: "contrast".to_string(),
                                    value: Some(Value::Int32(60)),
                                    ..Default::default()
                                })
                                .into(),
                                ..Default::default()
                            })
                            .into(),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
            )
            .await;

        let request = UpdateSettingsRequest {
            group_name: "display".to_string(),
            settings: vec![
                SettingProto {
                    key: "brightness".to_string(),
                    value: Some(Value::Int32(75)),
                    ..Default::default()
                },
                SettingProto {
                    key: "contrast".to_string(),
                    value: Some(Value::Int32(80)),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let response = service.UpdateSettings(get_user_controllable_service_fqin(), &request).await;
        assert_that!(response, ok(anything()));

        let settings_group = test_setup
            .persistence
            .setting_groups
            .get("display", &get_user_controllable_service_fqin().to_string())
            .expect("Failed to get settings group");
        let vehicle_settings = test_setup
            .persistence
            .vehicle_settings
            .get(settings_group.id)
            .expect("Failed to get vehicle settings");
        assert_eq!(vehicle_settings.len(), 2);
        assert_eq!(vehicle_settings[0].name, "brightness");
        assert_eq!(vehicle_settings[0].value, crate::persistence::setting::Value::Int32(75));
        assert_eq!(vehicle_settings[1].name, "contrast");
        assert_eq!(vehicle_settings[1].value, crate::persistence::setting::Value::Int32(80));
    }
}
