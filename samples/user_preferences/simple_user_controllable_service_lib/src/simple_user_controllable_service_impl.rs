// Copyright 2024 Google LLC
//! This module provides a generic implementation of the User Preferences UserControllableService interface.
//! It simplifies the process of handling user requests for setting changes and factory resets.

//! ## Key Features
//!     Accepts all incoming requests: The implementation accepts all requests for setting changes and factory resets, regardless of their validity.
//!     Upon receiving a request, the implementation modifies the stored value for the corresponding setting.
//!     Returns successful responses: The implementation always returns a successful response.
//!     Registers settings: The implementation automatically registers the settings provided by the service with the User Preferences Registry Service.
//!     Note: This implementation does not handle settings of kind PASSTRHOUGH
#![allow(missing_docs)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use itertools::Itertools;
use protobuf::MessageField;
use sdvgenerated_rpc_sdv_user_preferences_user_controllable_user_controllable_service::interface::UserControllableService;
use sdvgenerated_rpc_sdv_user_preferences_user_preferences_registry_service::{
    client::UserPreferencesRegistryServiceClient, interface::UserPreferencesRegistryService,
};
use user_preferences_api::user_controllable_service::{
    FactoryResetRequest, FactoryResetResponse, RequestSettingsChangeRequest,
    RequestSettingsChangeResponse,
};
use user_preferences_api::user_preferences_registry_service::{
    RegisterSettingsRequest, UpdateSettingsRequest,
};
use user_preferences_api::{
    error::{Error, ResponseCode},
    setting::{SettingAndConstraints, SettingDefinition},
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
    setting_and_constraints: HashMap<String, SettingAndConstraints>,
}

pub struct SimpleUserControllableServiceImpl {
    user_controllable_service_fqin: String,
    setting_groups_definitions: Vec<SettingGroupDefinition>,
    setting_groups: Mutex<HashMap<String, SettingGroup>>,
    user_preferences_registry_service_client: Arc<UserPreferencesRegistryServiceClient>,
}

impl SimpleUserControllableServiceImpl {
    pub fn new(
        user_controllable_service_fqin: String,
        setting_groups_definitions: Vec<SettingGroupDefinition>,
        user_preferences_registry_service_client: Arc<UserPreferencesRegistryServiceClient>,
    ) -> Self {
        let service_impl = Self {
            user_controllable_service_fqin,
            setting_groups_definitions,
            setting_groups: Mutex::new(HashMap::new()),
            user_preferences_registry_service_client,
        };

        service_impl.init_setting_groups();
        service_impl.register_settings();

        service_impl
    }

    fn init_setting_groups(&self) {
        let mut setting_groups = self.setting_groups.lock().unwrap();

        for group in self.setting_groups_definitions.iter().cloned() {
            setting_groups.insert(
                group.name.clone(),
                SettingGroup {
                    name: group.name,
                    setting_and_constraints: group
                        .setting_definitions
                        .into_iter()
                        .map(|setting_definition| {
                            let setting_and_constraint =
                                setting_definition.setting_and_constraints.into_option().unwrap();
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

    fn register_settings(&self) {
        for group in self.setting_groups_definitions.iter().cloned() {
            self.user_preferences_registry_service_client
                .register_settings(RegisterSettingsRequest {
                    group_name: group.name,
                    version: group.version,
                    settings_definitions: group.setting_definitions,
                    caller_fqin: self.user_controllable_service_fqin.clone(),
                    ..Default::default()
                })
                .expect("Failed to register settings");
        }
    }
}

impl UserControllableService for SimpleUserControllableServiceImpl {
    fn request_settings_change(
        &self,
        request: RequestSettingsChangeRequest,
    ) -> sdvmiddleware_rpc::RpcResult<RequestSettingsChangeResponse> {
        let mut setting_groups = self.setting_groups.lock().unwrap();

        let setting_group = match setting_groups.get_mut(&request.group_name) {
            Some(x) => x,
            None => {
                return Ok(RequestSettingsChangeResponse {
                    error: MessageField::some(Error {
                        error_code: ResponseCode::INVALID_REQUEST.into(),
                        message: format!(
                            "Setting group {} is not a valid group",
                            request.group_name
                        ),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
            }
        };
        for new_setting in request.settings.iter() {
            let persisted_setting = setting_group.setting_and_constraints.get_mut(&new_setting.key);
            persisted_setting.unwrap().setting = MessageField::some(new_setting.clone());
        }

        // Respond that all settings were applied as requested
        Ok(RequestSettingsChangeResponse { settings: request.settings, ..Default::default() })
    }

    fn factory_reset(
        &self,
        _request: FactoryResetRequest,
    ) -> sdvmiddleware_rpc::RpcResult<FactoryResetResponse> {
        self.init_setting_groups();

        let setting_groups = self.setting_groups.lock().unwrap();
        for setting_group in setting_groups.values() {
            let settings = setting_group
                .setting_and_constraints
                .values()
                .map(|setting| setting.setting.clone().unwrap())
                .collect_vec();

            self.user_preferences_registry_service_client
                .update_settings(UpdateSettingsRequest {
                    group_name: setting_group.name.clone(),
                    settings,
                    caller_fqin: self.user_controllable_service_fqin.clone(),
                    ..Default::default()
                })
                .expect("Unexpected failure to update settings");
        }

        Ok(FactoryResetResponse::new())
    }
}
