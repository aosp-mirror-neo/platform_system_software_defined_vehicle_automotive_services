// Copyright 2024 Google LLC
use async_trait::async_trait;
use sdv_comms::id::ServiceFqin;

#[async_trait]
pub trait OnlineServicesListener: Send + Sync {
    async fn on_service_start(&self, fqin: &ServiceFqin);
    async fn on_service_stop(&self, fqin: &ServiceFqin);
}
