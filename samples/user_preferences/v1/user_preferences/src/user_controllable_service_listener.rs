// Copyright 2024 Google LLC
use async_trait::async_trait;
use futures::StreamExt;
use log::error;
use log::info;
use sdv_comms::{
    id::ServiceFqin,
    sd::{ServiceDiscoveryManager, ServiceUnitChangeEvent, UnitChangeEventStream, UnitType},
};
use std::sync::Arc;

#[async_trait]
pub trait OnlineServicesListener: Send + Sync {
    async fn on_service_start(&self, fqin: &ServiceFqin);
    async fn on_service_stop(&self, fqin: &ServiceFqin);
}

pub async fn monitor_online_services(
    sd: Arc<ServiceDiscoveryManager>,
    listener: Arc<dyn OnlineServicesListener>,
) {
    tokio::spawn(async move {
        let unit_type = match UnitType::builder()
            .set_sdv_package_name("com.sdv.google.user_preferences")
            .set_service_bundle_name("UserPreferencesServiceBundle")
            .set_unit_type_name("user_controllable.UserControllableService")
            .build()
        {
            Ok(unit_type) => unit_type,
            Err(e) => {
                error!("Unable to build unit type: {e}");
                return;
            }
        };

        let mut unit_type_stream: UnitChangeEventStream =
            match sd.subscribe_service_unit_change_by_type(&unit_type).await {
                Ok(unit_type_stream) => unit_type_stream,
                Err(e) => {
                    error!("Unable to subscribe to service unit change: {e}");
                    return;
                }
            };

        while let Some(event) = unit_type_stream.next().await {
            let event: ServiceUnitChangeEvent = event;
            match event {
                ServiceUnitChangeEvent::Registered(sud) => {
                    let fqin = sud.get_service_bundle_identity().get_fqin().clone();
                    info!("Service Unit registered by {fqin:?}");

                    listener.on_service_start(&fqin).await;
                }
                ServiceUnitChangeEvent::Unregistered(sud) => {
                    let fqin = sud.get_service_bundle_identity().get_fqin().clone();
                    info!("Service Unit deregistered by {fqin:?}");

                    listener.on_service_stop(&fqin).await;
                }
                ServiceUnitChangeEvent::Changed(sud) => {
                    info!("Service Unit Definition change for {sud:?}")
                }
            }
        }
    });
}
