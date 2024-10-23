// Copyright 2024 Google LLC
//! Sample user preferences implementation

use crate::rpc_client_factory::RpcClientFactoryImpl;
use sdv_comms::id::ServiceFqin;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::Mutex;
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
}
