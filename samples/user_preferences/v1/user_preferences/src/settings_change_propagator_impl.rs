// Copyright 2024 Google LLC
//! Sample user preferences implementation

use crate::{
    persistence::{setting::Setting, Persistence},
    rpc_client_factory::RpcClientFactory,
    settings_retrieval_utils::{
        retrieve_user_settings_including_defaults_for_group,
        retrieve_vehicle_settings_including_defaults_for_group,
    },
    subscriptions_manager_impl::{SubscriptionsManager, SubscriptionsManagerImpl},
    user_controllable_service_listener::OnlineServicesListener,
};
use async_trait::async_trait;
use itertools::Itertools;
use log::{error, info};
use mockall::automock;
use sdv_comms::id::ServiceFqin;
use std::collections::HashSet;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use user_preferences_api::setting::Setting as SettingProto;
use user_preferences_api::{
    settings_group::SettingsGroupId,
    user_controllable_service::{RequestSettingsChangeRequest, RequestSettingsChangeResponse},
};

#[automock]
#[async_trait]
pub trait SettingsChangePropagator: Send + Sync {
    async fn queue_setting_change(
        &self,
        fqin: ServiceFqin,
        group_name: String,
        settings: Vec<Setting>,
    );

    async fn get_pending_setting_changes(
        &self,
        fqin: &ServiceFqin,
        group_name: &str,
    ) -> Vec<Setting>;

    async fn is_service_available(&self, fqin: &ServiceFqin) -> bool;

    async fn notify_subscribers_of_current_state(
        &self,
        fqin: &ServiceFqin,
        group_name: &str,
        updated_settings_names: &HashSet<String>,
    );
}

pub struct SettingsChangePropagatorImpl {
    persistence: Arc<Persistence>,
    rpc_client_factory: Arc<dyn RpcClientFactory>,
    pending_setting_changes: Mutex<HashMap<ServiceFqin, ServiceSettingPendingChanges>>,
    online_services: Arc<Mutex<HashSet<ServiceFqin>>>,
    subscriptions_manager_impl: Arc<SubscriptionsManagerImpl>,
}

impl SettingsChangePropagatorImpl {
    pub fn new(
        persistence: Arc<Persistence>,
        rpc_client_factory: Arc<dyn RpcClientFactory>,
        subscriptions_manager_impl: Arc<SubscriptionsManagerImpl>,
    ) -> Self {
        Self {
            persistence,
            rpc_client_factory,
            pending_setting_changes: Mutex::new(HashMap::new()),
            online_services: Arc::new(Mutex::new(HashSet::new())),
            subscriptions_manager_impl,
        }
    }

    async fn process_pending_setting_changes(&self, fqin: &ServiceFqin) {
        let client = match self.rpc_client_factory.new_user_controllable_service_client(fqin).await
        {
            Ok(client) => client,
            Err(err) => {
                error!("Failed to create RPC client for {}: {}", fqin, err);
                return;
            }
        };
        let mut pending_setting_changes = self.pending_setting_changes.lock().await;
        let service_pending_settings = match pending_setting_changes.remove(fqin) {
            Some(x) => x,
            None => return,
        };
        drop(pending_setting_changes);

        for (group_name, settings) in service_pending_settings.into_iter() {
            let group_id = match self.persistence.setting_groups.get(&group_name, &fqin.to_string())
            {
                Ok(x) => x.id,
                Err(err) => {
                    error!(
                        "Failed to retrieve settings group primary key id for {}: {}",
                        fqin, err
                    );
                    // notify subscribers of the latest state of these settings
                    self.notify_subscribers_of_current_state(
                        fqin,
                        &group_name,
                        &settings
                            .iter()
                            .map(|setting| setting.name.clone())
                            .collect::<HashSet<String>>(),
                    )
                    .await;
                    continue;
                }
            };

            let response: RequestSettingsChangeResponse = match client
                .RequestSettingsChange(&RequestSettingsChangeRequest {
                    group_name: group_name.clone(),
                    settings: settings.iter().cloned().map_into().collect(),
                    ..Default::default()
                })
                .await
            {
                Ok(response) => response,
                Err(err) => {
                    error!(
                        "RequestSettingsChange RPC to {} to update group {} failed: {}",
                        fqin.to_string(),
                        group_name.clone(),
                        err
                    );
                    self.notify_subscribers_of_current_state(
                        fqin,
                        &group_name,
                        &settings
                            .iter()
                            .map(|setting| setting.name.clone())
                            .collect::<HashSet<String>>(),
                    )
                    .await;
                    continue;
                }
            };

            // Update vehicle settings if the RPC call was successful
            if let Err(err) = self.persistence.vehicle_settings.store_all(
                // Assuming you have a way to get group ID from group name
                // Replace this with your actual logic
                group_id,
                response.settings.iter().cloned().map_into().collect(),
            ) {
                error!("Failed to store vehicle settings for {}/{}: {}", fqin, group_name, err);
            } else {
                info!("Successfully applied settings for {}/{}", fqin.to_string(), group_name);
            }
            self.notify_subscribers_of_current_state(
                fqin,
                &group_name,
                &settings.into_iter().map(|setting| setting.name).collect::<HashSet<String>>(),
            )
            .await;
        }
    }

    // When a UserControllableService comes online:
    // 1. Check if there's an active user.
    // 2. If so, queue the user's preferences for this service,
    //    ensuring we don't duplicate any settings already in the pending queue.
    async fn init_service_with_user_preferences(&self, fqin: &ServiceFqin) {
        let active_user = match self.persistence.users.get_active_user() {
            Ok(x) => x,
            Err(err) => {
                error!("Unable to retrieve the active user: {}", err);
                return;
            }
        };
        let active_user = match active_user {
            Some(x) => x,
            None => return,
        };

        let setting_groups = match self.persistence.setting_groups.list() {
            Ok(x) => x,
            Err(err) => {
                error!("Unable to retrieve list of setting groups: {}", err);
                return;
            }
        };

        for setting_group in setting_groups {
            let pending_settings =
                self.get_pending_setting_changes(fqin, &setting_group.name).await;
            let pending_settings = pending_settings
                .into_iter()
                .map(|setting| setting.name)
                .collect::<HashSet<String>>();

            let settings = match retrieve_user_settings_including_defaults_for_group(
                &self.persistence.setting_definitions,
                &self.persistence.user_settings,
                active_user.id,
                setting_group.id,
            ) {
                Ok(x) => x,
                Err(err) => {
                    error!("Unable to retrieve the user preferences: {}", err);
                    return;
                }
            };
            let settings_to_queue = settings
                .into_iter()
                .filter(|setting| !pending_settings.contains(&setting.name))
                .collect_vec();

            self.queue_setting_change(fqin.clone(), setting_group.name, settings_to_queue).await;
        }
    }
}

#[async_trait]
impl SettingsChangePropagator for SettingsChangePropagatorImpl {
    async fn queue_setting_change(
        &self,
        fqin: ServiceFqin,
        group_name: String,
        settings: Vec<Setting>,
    ) {
        let mut pending_setting_changes = self.pending_setting_changes.lock().await;
        pending_setting_changes
            .entry(fqin.clone())
            .or_default()
            .add_setting_change(group_name.clone(), settings.clone());
        drop(pending_setting_changes);

        if self.is_service_available(&fqin).await {
            self.process_pending_setting_changes(&fqin).await;
        } else {
            // If the service is not online, then notify all settings change subscribers
            // that the settings are in a pending state
            // notify subscribers of the latest state of these settings
            self.notify_subscribers_of_current_state(
                &fqin,
                &group_name,
                &settings.iter().map(|setting| setting.name.clone()).collect::<HashSet<String>>(),
            )
            .await;
        }
    }

    async fn get_pending_setting_changes(
        &self,
        fqin: &ServiceFqin,
        group_name: &str,
    ) -> Vec<Setting> {
        let pending_setting_changes = self.pending_setting_changes.lock().await;
        let service_pending_changes = match pending_setting_changes.get(fqin) {
            Some(x) => x,
            None => return Vec::new(),
        };
        service_pending_changes.get_setting_change(&group_name.to_string())
    }

    async fn is_service_available(&self, fqin: &ServiceFqin) -> bool {
        let online_services = self.online_services.lock().await;
        online_services.contains(fqin)
    }

    async fn notify_subscribers_of_current_state(
        &self,
        fqin: &ServiceFqin,
        group_name: &str,
        updated_settings_names: &HashSet<String>,
    ) {
        let settings_group_primary_key =
            match self.persistence.setting_groups.get(group_name, &fqin.to_string()) {
                Ok(x) => x.id,
                Err(err) => {
                    error!(
                    "SubscribeToSettingsChangeAndGetSettings failed to retrieve settings group: {}",
                    err
                );
                    return;
                }
            };

        let pending_settings: Vec<SettingProto> = self
            .get_pending_setting_changes(fqin, group_name)
            .await
            .into_iter()
            .map_into()
            .collect();
        let active_settings = match retrieve_vehicle_settings_including_defaults_for_group(
            &self.persistence.setting_definitions,
            &self.persistence.vehicle_settings,
            settings_group_primary_key,
        ) {
            Ok(x) => x,
            Err(err) => {
                error!("Unable to notify clients of settings change due to: {}", err);
                return;
            }
        };
        let active_settings = active_settings
            .into_iter()
            .filter_map(|setting_with_constraints| {
                let setting = setting_with_constraints.setting.unwrap();
                match updated_settings_names.contains(&setting.key) {
                    true => Some(setting),
                    false => None,
                }
            })
            .collect_vec();

        let active_user = match self.persistence.users.get_active_user() {
            Ok(x) => x,
            Err(err) => {
                error!("Unable to notify clients of settings change due to error retrieving active user: {}", err);
                return;
            }
        };

        let persisted_settings: Vec<SettingProto> = if let Some(active_user) = active_user {
            match retrieve_user_settings_including_defaults_for_group(
                &self.persistence.setting_definitions,
                &self.persistence.user_settings,
                active_user.id,
                settings_group_primary_key,
            ) {
                Ok(x) => x.into_iter().map_into().collect(),
                Err(err) => {
                    error!("Unable to notify clients of settings change due to: {}", err);
                    return;
                }
            }
        } else {
            Vec::new()
        };
        let persisted_settings = persisted_settings
            .into_iter()
            .filter_map(|setting| match updated_settings_names.contains(&setting.key) {
                true => Some(setting),
                false => None,
            })
            .collect_vec();

        self.subscriptions_manager_impl
            .notify_subscribers(
                &SettingsGroupId {
                    name: group_name.to_string(),
                    service_fqin: fqin.to_string(),
                    ..Default::default()
                },
                &active_settings,
                &pending_settings,
                &persisted_settings,
            )
            .await;
    }
}

#[async_trait]
impl OnlineServicesListener for SettingsChangePropagatorImpl {
    async fn on_service_start(&self, fqin: &ServiceFqin) {
        let mut online_services = self.online_services.lock().await;
        online_services.insert(fqin.clone());
        drop(online_services);

        self.init_service_with_user_preferences(fqin).await;
    }

    async fn on_service_stop(&self, fqin: &ServiceFqin) {
        let mut online_services = self.online_services.lock().await;
        online_services.remove(fqin);
    }
}

#[derive(Default)]
struct ServiceSettingPendingChanges {
    setting_changes: HashMap<String, HashMap<String, Setting>>,
}

impl ServiceSettingPendingChanges {
    pub fn add_setting_change(&mut self, group_name: String, settings: Vec<Setting>) {
        let setting_changes = self.setting_changes.entry(group_name).or_default();
        for setting in settings.into_iter() {
            setting_changes.insert(setting.name.clone(), setting);
        }
    }
    pub fn get_setting_change(&self, group_name: &String) -> Vec<Setting> {
        // We need to deduplicate settings, taking the last setting found from the group
        let pending_settings = self.setting_changes.get(group_name).cloned().unwrap_or_default();

        pending_settings.into_values().collect()
    }
}

impl IntoIterator for ServiceSettingPendingChanges {
    type Item = (String, Vec<Setting>);
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.setting_changes
            .into_iter()
            .map(|(group_name, settings)| (group_name, settings.into_values().collect()))
            .collect::<Vec<_>>()
            .into_iter()
    }
}
