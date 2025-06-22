// Copyright 2024 Google LLC
//! This library provides the base implementation of the User Controllable Service interface used by the car_seats_service and hvac_service sample bundles.

use async_trait::async_trait;
use com_sdv_google_user_preferences_user_controllable_user_controllable_service_rpc::Interface as UserControllableService;
use com_sdv_google_user_preferences_user_preferences_registry_service_rpc::Client as RegistryServiceClient;
use protobuf::MessageField;
use sdv_status_rs::SdvResult;
use sdv_status_rs::SdvStatus;
use sdv_status_rs::SdvStatusCode;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use user_preferences_api::{
    setting::{SettingDefinition, SettingWithConstraints},
    user_controllable_service::{FactoryResetResponse, RequestSettingsChangeResponse},
    user_preferences_registry_service::{RegisterSettingsRequest, UpdateSettingsRequest},
};

/// Definition of the settings group a service provides to users of a vehicle.
/// Examples of the SettingGRoupDefinition usage can be found in the car seats
/// service bundle automotive_services/samples/user_preferences/v1/car_seats_service/src/main.rs
#[derive(Clone)]
pub struct SettingGroupDefinition {
    /// Name of the settings group.
    pub name: String,
    /// Version of the schema of this group.
    pub version: String,
    /// The definition of the settings within this group.
    pub setting_definitions: Vec<SettingDefinition>,
}

#[derive(Clone)]
struct SettingGroup {
    name: String,
    setting_with_constraints: HashMap<String, SettingWithConstraints>,
}

/// Implements the `UserControllableService` interface, providing a simple service bundle that
/// accepts all setting change requests and registers settings within the User Preferences service.
/// OEM services must inform the User Preferences service of the settings they support.  This is done by calling
/// the register settings API exposed by the User Preferences service.  This service bundle automatically performs this
/// registration.
pub struct SimpleUserControllableServiceImpl {
    setting_groups_definitions: Vec<SettingGroupDefinition>,
    setting_groups: Mutex<HashMap<String, SettingGroup>>,
    client: Arc<RegistryServiceClient>,
}

impl SimpleUserControllableServiceImpl {
    /// Creates a new instance of `SimpleUserControllableServiceImpl`.
    ///
    /// # Arguments
    /// * `setting_groups_definitions` - A vector of `SettingGroupDefinition` structs defining the settings groups managed by this service.
    /// * `client` - A RegistryServiceClient used to communicate with the settings registry.
    ///
    /// # Returns
    /// A new `SimpleUserControllableServiceImpl` instance.  This function also initializes the setting groups
    /// and registers the defined settings with the User Preferences service.
    pub async fn new(
        setting_groups_definitions: Vec<SettingGroupDefinition>,
        client: Arc<RegistryServiceClient>,
    ) -> SdvResult<Self> {
        let service_impl =
            Self { setting_groups_definitions, setting_groups: Mutex::new(HashMap::new()), client };

        service_impl.init_setting_groups().await?;
        service_impl.register_settings().await?;

        Ok(service_impl)
    }

    async fn init_setting_groups(&self) -> SdvResult<()> {
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
                            let setting_and_constraint = setting_definition
                                .setting_with_constraints
                                .into_option()
                                .ok_or(SdvStatus::new(SdvStatusCode::NotFound))?;
                            Ok((
                                setting_and_constraint
                                    .setting
                                    .as_ref()
                                    .ok_or(SdvStatus::new(SdvStatusCode::NotFound))?
                                    .key
                                    .clone(),
                                setting_and_constraint,
                            ))
                        })
                        .collect::<SdvResult<HashMap<_, _>>>()?,
                },
            );
        }

        Ok(())
    }

    async fn register_settings(&self) -> SdvResult<()> {
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
                .map_err(|e| {
                    Err(SdvStatus::with_message(
                        sdv_status_rs::SdvStatusCode::Internal,
                        format!("Failed to register settings: {e}"),
                    ))
                })?;
        }
        Ok(())
    }
}

#[async_trait]
impl UserControllableService for SimpleUserControllableServiceImpl {
    /// Handles requests to change settings.  This function retrieves the relevant setting group,
    /// updates the settings values, and sends a response indicating whether the changes were applied.
    ///
    /// # Arguments
    /// * `_caller_id` - The identifier of the calling service.
    /// * `request` - The `RequestSettingsChangeRequest` containing the settings to be changed.
    ///
    /// # Returns
    /// An `SdvResult` containing a `RequestSettingsChangeResponse` with the applied settings,
    /// or an error if the setting group is not found.
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
            let persisted_setting = setting_group
                .setting_with_constraints
                .get_mut(&new_setting.key)
                .ok_or(SdvStatus::new(SdvStatusCode::NotFound))?;
            persisted_setting.setting = MessageField::some(new_setting.clone());

            println!("Key {:<15} Value {:<10?}", new_setting.key, new_setting.value)
        }

        // Respond that all settings were applied as requested
        Ok(RequestSettingsChangeResponse {
            settings: request.settings.clone(),
            ..Default::default()
        })
    }

    /// Handles factory reset requests.  This function resets the settings to their default values
    /// as defined in the `SettingGroupDefinition` and updates the registry.
    ///
    /// # Arguments
    /// * `_caller_id` - The identifier of the calling service.
    /// * `_request` - The `FactoryResetRequest`.
    ///
    /// # Returns
    /// An `SdvResult` containing a `FactoryResetResponse`.
    async fn FactoryReset(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        _request: &user_preferences_api::user_controllable_service::FactoryResetRequest,
    ) -> sdv_status_rs::SdvResult<
        user_preferences_api::user_controllable_service::FactoryResetResponse,
    > {
        self.init_setting_groups().await?;

        let setting_groups = self.setting_groups.lock().await;
        for setting_group in setting_groups.values() {
            let settings = setting_group
                .setting_with_constraints
                .values()
                .map(|setting| {
                    setting
                        .setting
                        .clone()
                        .into_option()
                        .ok_or(SdvStatus::new(SdvStatusCode::NotFound))
                })
                .collect::<SdvResult<Vec<_>>>()?;

            self.client
                .UpdateSettings(&UpdateSettingsRequest {
                    group_name: setting_group.name.clone(),
                    settings,
                    ..Default::default()
                })
                .await
                .map_err(|e| {
                    Err(SdvStatus::with_message(
                        sdv_status_rs::SdvStatusCode::Internal,
                        format!("Unexpected failure to update settings: {e}"),
                    ))
                })?;

            println!("\nPerformed factory reset");
        }
        Ok(FactoryResetResponse::new())
    }
}
