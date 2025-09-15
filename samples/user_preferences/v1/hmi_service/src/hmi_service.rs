// Copyright 2024 Google LLC
use async_trait::async_trait;
use com_sdv_google_user_preferences_view_change_notifier_rpc::Interface as ViewChangeNotifier;
use log::error;
use log::info;
use tokio::sync::{mpsc, Mutex};
use user_preferences_api::change_notifier::{OnSettingsChangeRequest, OnSettingsChangeResponse};
use user_preferences_utils::print_settings;

pub struct HmiServiceImpl {
    sender: mpsc::Sender<()>,
    receiver: Mutex<mpsc::Receiver<()>>,
}

impl HmiServiceImpl {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(1);
        Self { sender, receiver: Mutex::new(receiver) }
    }

    pub async fn wait_for_settings_change(&self) {
        self.receiver.lock().await.recv().await;
    }
}

#[async_trait]
impl ViewChangeNotifier for HmiServiceImpl {
    async fn OnSettingsChange(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        request: &OnSettingsChangeRequest,
    ) -> sdv_status_rs::SdvResult<OnSettingsChangeResponse> {
        if request.settings_group_id.is_some() {
            info!(
                "\n\nThe following settings were changed in settings group {}/{}",
                request.settings_group_id.service_fqin, request.settings_group_id.name
            );
            print_settings(&request.active_settings);
        }

        // Send a notification that settings have changed.
        if let Err(e) = self.sender.try_send(()) {
            error!("Try send error: {e}");
        };

        Ok(OnSettingsChangeResponse::new())
    }
}
