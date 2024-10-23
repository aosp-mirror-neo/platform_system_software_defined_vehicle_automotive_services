// Copyright 2024 Google LLC
//! Sample user preferences implementation

use crate::rpc_client_factory::RpcClientFactoryImpl;
use log::error;
use sdv_comms::id::ServiceFqin;
use sdv_user_preferences_view_change_notifier_rpc::Client as ChangeNotifierClient;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::Mutex;
use user_preferences_api::change_notifier::OnSettingsChangeRequest;
use user_preferences_api::setting::Setting as SettingProto;
use user_preferences_api::settings_group::SettingsGroupId;

#[derive(Debug, Hash, PartialEq, Eq)]
struct SubscriptionKey {
    service_fqin: String,
    group_name: String,
}

impl From<SettingsGroupId> for SubscriptionKey {
    fn from(value: SettingsGroupId) -> Self {
        Self { group_name: value.name, service_fqin: value.service_fqin }
    }
}

pub struct SubscriptionsManagerImpl {
    rpc_client_factory: Arc<RpcClientFactoryImpl>,
    subscribers: Mutex<HashMap<SubscriptionKey, HashSet<ServiceFqin>>>,
}

impl SubscriptionsManagerImpl {
    pub fn new(rpc_client_factory: Arc<RpcClientFactoryImpl>) -> Self {
        Self { rpc_client_factory, subscribers: tokio::sync::Mutex::new(HashMap::new()) }
    }

    pub async fn register_subscriber(&self, group: SettingsGroupId, subscriber_fqin: ServiceFqin) {
        let mut subscribers = self.subscribers.lock().await;
        subscribers.entry(group.into()).or_default().insert(subscriber_fqin);
    }

    pub async fn unregister_subscriber(
        &self,
        group: SettingsGroupId,
        subscriber_fqin: ServiceFqin,
    ) {
        let mut subscribers = self.subscribers.lock().await;
        subscribers.entry(group.into()).and_modify(|f| {
            f.remove(&subscriber_fqin);
        });
    }

    pub async fn notify_subscribers(
        &self,
        group: &SettingsGroupId,
        active_settings: &[SettingProto],
        pending_changes: &[SettingProto],
        persisted_settings: &[SettingProto],
    ) {
        let subscribers = self.subscribers.lock().await;
        let subscribers = match subscribers.get(&group.clone().into()) {
            Some(x) => x,
            None => return,
        };

        for subscriber in subscribers {
            let client: ChangeNotifierClient = match self
                .rpc_client_factory
                .new_change_notifier_client(
                    subscriber.get_sdv_package_name(),
                    subscriber.get_service_bundle_name(),
                )
                .await
            {
                Ok(client) => client,
                Err(err) => {
                    error!("Failed to create rpc client for {}: {}", subscriber, err);
                    continue;
                }
            };
            if let Err(err) = client
                .OnSettingsChange(&OnSettingsChangeRequest {
                    settings_group_id: Some(SettingsGroupId {
                        service_fqin: group.service_fqin.to_string(),
                        name: group.name.to_string(),
                        ..Default::default()
                    })
                    .into(),
                    active_settings: active_settings.to_vec(),
                    pending_changes: pending_changes.to_vec(),
                    persisted_settings: persisted_settings.to_vec(),
                    ..Default::default()
                })
                .await
            {
                error!(
                    "Failed to create notify {} of settings change: {}",
                    group.service_fqin, err
                );
            }
        }
    }
}
