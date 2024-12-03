// Copyright 2024 Google LLC
//! Sample user preferences implementation

use crate::persistence::setting::Setting;
use crate::persistence::setting_definition::SettingDefinition;
use crate::persistence::settings_group_repository::SettingsGroup;
use crate::persistence::Persistence;
use crate::settings_change_propagator_impl::SettingsChangePropagator;
use crate::settings_retrieval_utils::{
    retrieve_user_settings_including_defaults_for_group,
    retrieve_vehicle_settings_including_defaults_for_group,
};
use crate::subscriptions_manager_impl::SubscriptionsManager;
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
    settings_change_propagator: Arc<dyn SettingsChangePropagator>,
    subscriptions_manager: Arc<dyn SubscriptionsManager>,
}

impl ManagementServiceImpl {
    pub fn new(
        persistence: Arc<Persistence>,
        settings_change_propagator: Arc<dyn SettingsChangePropagator>,
        subscriptions_manager: Arc<dyn SubscriptionsManager>,
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
                return Err(err);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{persistence::user_repository::User, test_utils::*};
    use googletest::prelude::*;
    use protobuf::Enum;
    use std::collections::HashMap;
    use user_preferences_api::setting::Setting as SettingProto;
    use user_preferences_api::settings_group::SettingsGroupId as SettingsGroupIdProto;
    use user_preferences_api::user::user::UserFlags;
    use user_preferences_api::user_preferences_management_service::request_settings_change_request::ChangePersistencePolicy;
    use user_preferences_api::user_preferences_management_service::RequestSettingsChangeRequest;

    #[tokio::test]
    async fn request_settings_change_returns_error_for_non_existent_group() {
        let test_setup = TestSetup::new();
        let service = create_service_impl(&test_setup);

        let request = RequestSettingsChangeRequest {
            settings_group_id: Some(SettingsGroupIdProto {
                name: "nonexistent_group".to_string(),
                service_fqin: get_user_controllable_service_fqin().to_string(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        };

        let response = service.RequestSettingsChange(get_client_fqin(), &request).await;

        assert_that!(response, err(anything()));
        assert_eq!(response.unwrap_err().status_code(), SdvStatusCode::NotFound);
    }

    #[tokio::test]
    async fn request_settings_change_returns_error_for_invalid_settings() {
        let test_setup = TestSetup::new();
        let (group, _) = test_setup.populate_persistence_with_settings_definitions();
        let service = create_service_impl(&test_setup);

        let user_id = 1;
        test_setup
            .persistence
            .users
            .add_user(&User { id: user_id, flags: UserFlags::DRIVER.value() })
            .expect("Failed to create user");
        test_setup.persistence.users.set_active_user(user_id).expect("Failed to select user");

        let request = RequestSettingsChangeRequest {
            settings_group_id: Some(SettingsGroupIdProto {
                name: group.name.clone(),
                service_fqin: group.service_fqin.clone(),
                ..Default::default()
            })
            .into(),
            settings: vec![SettingProto {
                key: "brightness".to_string(),
                value: Some(user_preferences_api::setting::setting::Value::Int32(125)),
                ..Default::default()
            }],
            ..Default::default()
        };

        let response = service.RequestSettingsChange(get_client_fqin(), &request).await;

        assert_that!(response, err(anything()));
        assert_eq!(response.unwrap_err().status_code(), SdvStatusCode::InvalidArgument);
    }

    #[tokio::test]
    async fn request_settings_change_succeeds_for_valid_settings() {
        let mut test_setup = TestSetup::new();
        let (group, _) = test_setup.populate_persistence_with_settings_definitions();

        let user_id = 1;
        test_setup
            .persistence
            .users
            .add_user(&User { id: user_id, flags: UserFlags::DRIVER.value() })
            .expect("Failed to create user");
        test_setup.persistence.users.set_active_user(user_id).expect("Failed to select user");

        let expected_group_name = group.name.clone();
        let expected_fqin = group.service_fqin.clone();
        test_setup
            .mut_settings_change_propagator()
            .expect_queue_setting_change()
            .withf(move |fqin, group_name, settings| {
                fqin.to_string() == expected_fqin
                    && group_name == &expected_group_name
                    && settings.len() == 1
                    && settings[0].name == "brightness"
            })
            .returning(|_, _, _| ());

        let service = create_service_impl(&test_setup);

        let request = RequestSettingsChangeRequest {
            settings_group_id: Some(SettingsGroupIdProto {
                name: group.name.clone(),
                service_fqin: group.service_fqin.clone(),
                ..Default::default()
            })
            .into(),
            settings: vec![SettingProto {
                key: "brightness".to_string(),
                value: Some(user_preferences_api::setting::setting::Value::Int32(60)),
                ..Default::default()
            }],
            ..Default::default()
        };

        let response = service.RequestSettingsChange(get_client_fqin(), &request).await;

        assert_that!(response, ok(anything()));
    }

    #[tokio::test]
    async fn request_settings_change_persists_settings_for_selected_user() {
        let mut test_setup = TestSetup::new();
        let (group, definitions) = test_setup.populate_persistence_with_settings_definitions();

        let expected_group_name = group.name.clone();
        let expected_fqin = group.service_fqin.clone();
        let definitions_list_length = definitions.len();
        test_setup
            .mut_settings_change_propagator()
            .expect_queue_setting_change()
            .withf(move |fqin, group_name, settings| {
                fqin.to_string() == expected_fqin
                    && group_name == &expected_group_name
                    && settings.len() == definitions_list_length
            })
            .returning(|_, _, _| ());

        let service = create_service_impl(&test_setup);
        let user_id = 1;
        test_setup
            .persistence
            .users
            .add_user(&User { id: user_id, flags: UserFlags::DRIVER.value() })
            .expect("Failed to create user");
        test_setup.persistence.users.set_active_user(user_id).expect("Failed to select user");

        let request = RequestSettingsChangeRequest {
            settings_group_id: Some(SettingsGroupIdProto {
                name: group.name.clone(),
                service_fqin: group.service_fqin.clone(),
                ..Default::default()
            })
            .into(),
            settings: vec![
                SettingProto {
                    key: "brightness".to_string(),
                    value: Some(user_preferences_api::setting::setting::Value::Int32(75)),
                    ..Default::default()
                },
                SettingProto {
                    key: "contrast".to_string(),
                    value: Some(user_preferences_api::setting::setting::Value::Int32(32)),
                    ..Default::default()
                },
            ],
            change_persistence_policy: ChangePersistencePolicy::PERSISTENT_CHANGE.into(),
            ..Default::default()
        };

        let response = service.RequestSettingsChange(get_client_fqin(), &request).await;
        assert_that!(response, ok(anything()));

        let stored_settings = test_setup
            .persistence
            .user_settings
            .get_all(user_id, group.id)
            .expect("Failed to retrieve user settings");

        assert_eq!(stored_settings.len(), definitions_list_length);
        let mut settings_map = HashMap::new();
        for setting in stored_settings.iter() {
            settings_map.insert(setting.name.clone(), setting);
        }

        assert_eq!(
            settings_map.get("brightness").unwrap().value,
            crate::persistence::setting::Value::Int32(75)
        );
        assert_eq!(
            settings_map.get("contrast").unwrap().value,
            crate::persistence::setting::Value::Int32(32)
        );
    }

    #[tokio::test]
    async fn request_settings_change_does_not_persist_settings_when_no_user_selected() {
        let mut test_setup = TestSetup::new();
        let (group, definitions) = test_setup.populate_persistence_with_settings_definitions();

        let expected_group_name = group.name.clone();
        let expected_fqin = group.service_fqin.clone();
        let definitions_array_length = definitions.len();
        test_setup
            .mut_settings_change_propagator()
            .expect_queue_setting_change()
            .withf(move |fqin, group_name, settings| {
                fqin.to_string() == expected_fqin
                    && group_name == &expected_group_name
                    && settings.len() == definitions_array_length
            })
            .returning(|_, _, _| ());

        let service = create_service_impl(&test_setup);

        let request = RequestSettingsChangeRequest {
            settings_group_id: Some(SettingsGroupIdProto {
                name: group.name.clone(),
                service_fqin: group.service_fqin.clone(),
                ..Default::default()
            })
            .into(),
            settings: vec![
                SettingProto {
                    key: "brightness".to_string(),
                    value: Some(user_preferences_api::setting::setting::Value::Int32(75)),
                    ..Default::default()
                },
                SettingProto {
                    key: "contrast".to_string(),
                    value: Some(user_preferences_api::setting::setting::Value::Int32(32)),
                    ..Default::default()
                },
            ],
            change_persistence_policy: ChangePersistencePolicy::PERSISTENT_CHANGE.into(),
            ..Default::default()
        };

        let response = service.RequestSettingsChange(get_client_fqin(), &request).await;
        assert_that!(response, err(anything()));
        assert_eq!(response.unwrap_err().status_code(), SdvStatusCode::Internal);
    }

    #[tokio::test]
    async fn subscribe_to_settings_change_and_get_settings_returns_error_for_non_existent_group() {
        let test_setup = TestSetup::new();
        let service = create_service_impl(&test_setup);

        let request = SubscribeToSettingsChangeAndGetSettingsRequest {
            settings_group_id: Some(SettingsGroupIdProto {
                name: "nonexistent_group".to_string(),
                service_fqin: get_user_controllable_service_fqin().to_string(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        };

        let response =
            service.SubscribeToSettingsChangeAndGetSettings(get_client_fqin(), &request).await;

        assert_that!(response, err(anything()));
        assert_eq!(response.unwrap_err().status_code(), SdvStatusCode::NotFound);
    }

    #[tokio::test]
    async fn subscribe_to_settings_change_and_get_settings_returns_active_settings_when_no_user_is_selected(
    ) {
        let mut test_setup = TestSetup::new();
        let (group, definitions) = test_setup.populate_persistence_with_settings_definitions();

        let expected_group_name = group.name.clone();
        let expected_fqin = group.service_fqin.clone();
        test_setup
            .mut_settings_change_propagator()
            .expect_get_pending_setting_changes()
            .withf(move |fqin, name| {
                fqin.to_string() == expected_fqin && name == expected_group_name
            })
            .returning(|_, _| Vec::new());

        let expected_group_name = group.name.clone();
        test_setup
            .mut_subscriptions_manager()
            .expect_register_subscriber()
            .withf(move |group_id, _caller_id| group_id.name == expected_group_name)
            .returning(|_, _| ());

        let request = SubscribeToSettingsChangeAndGetSettingsRequest {
            settings_group_id: Some(SettingsGroupIdProto {
                name: group.name.clone(),
                service_fqin: group.service_fqin.clone(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        };

        let service = create_service_impl(&test_setup);
        let response =
            service.SubscribeToSettingsChangeAndGetSettings(get_client_fqin(), &request).await;
        assert_that!(response, ok(anything()));

        let response = response.unwrap();
        assert_eq!(response.active_settings.len(), definitions.len());
        assert_eq!(response.pending_settings.len(), 0);
        assert_eq!(response.persisted_settings.len(), 0);
        assert_eq!(response.version, group.version);
    }

    #[tokio::test]
    async fn settings_subscription_includes_persisted_user_settings() {
        let mut test_setup = TestSetup::new();
        let (group, definitions) = test_setup.populate_persistence_with_settings_definitions();

        let expected_group_name = group.name.clone();
        let expected_fqin = group.service_fqin.clone();
        test_setup
            .mut_settings_change_propagator()
            .expect_get_pending_setting_changes()
            .withf(move |fqin, name| {
                fqin.to_string() == expected_fqin && name == expected_group_name
            })
            .returning(|_, _| Vec::new());

        let expected_group_name = group.name.clone();
        test_setup
            .mut_subscriptions_manager()
            .expect_register_subscriber()
            .withf(move |group_id, _caller_id| group_id.name == expected_group_name)
            .returning(|_, _| ());

        let user_id = 1;
        test_setup
            .persistence
            .users
            .add_user(&User { id: user_id, flags: UserFlags::DRIVER.value() })
            .expect("Failed to create user");
        test_setup.persistence.users.set_active_user(user_id).expect("Failed to select user");

        // Set a user setting
        let user_settings = vec![
            Setting {
                name: "brightness".to_string(),
                value: crate::persistence::setting::Value::Int32(75),
            },
            Setting {
                name: "contrast".to_string(),
                value: crate::persistence::setting::Value::Int32(32),
            },
        ];

        test_setup
            .persistence
            .user_settings
            .store_all(user_id, group.id, &user_settings)
            .expect("Failed to store user settings");

        let request = SubscribeToSettingsChangeAndGetSettingsRequest {
            settings_group_id: Some(SettingsGroupIdProto {
                name: group.name.clone(),
                service_fqin: group.service_fqin.clone(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        };

        let service = create_service_impl(&test_setup);
        let response = service
            .SubscribeToSettingsChangeAndGetSettings(get_client_fqin(), &request)
            .await
            .unwrap();

        // Check persisted settings are returned
        assert_eq!(response.persisted_settings.len(), definitions.len());
        let mut settings_map = HashMap::new();
        for setting in response.persisted_settings.iter() {
            settings_map.insert(setting.key.clone(), setting);
        }

        for expected_setting in user_settings {
            assert!(settings_map.contains_key(&expected_setting.name));
            assert_eq!(
                settings_map.get(&expected_setting.name).as_ref().unwrap().value.clone().unwrap(),
                expected_setting.value.into()
            );
        }
    }

    #[tokio::test]
    async fn unsubscribe_from_settings_change_succeeds() {
        let mut test_setup = TestSetup::new();
        let (group, _) = test_setup.populate_persistence_with_settings_definitions();

        let expected_group_name = group.name.clone();
        test_setup
            .mut_subscriptions_manager()
            .expect_unregister_subscriber()
            .withf(move |group_id, caller_id| {
                group_id.name == expected_group_name && caller_id == &get_client_fqin()
            })
            .returning(|_, _| ());

        let service = create_service_impl(&test_setup);

        let request = UnsubscribeToSettingsChangeRequest {
            settings_group_id: Some(SettingsGroupIdProto {
                name: group.name.clone(),
                service_fqin: group.service_fqin.clone(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        };

        let response = service.UnsubscribeFromSettingsChange(get_client_fqin(), &request).await;
        assert_that!(response, ok(anything()));
    }

    pub fn create_service_impl(test_setup: &TestSetup) -> ManagementServiceImpl {
        ManagementServiceImpl::new(
            test_setup.persistence.clone(),
            test_setup.settings_change_propagator.clone(),
            test_setup.subscriptions_manager.clone(),
        )
    }
}
