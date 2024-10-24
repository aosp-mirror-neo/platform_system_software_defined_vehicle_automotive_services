// Copyright 2024 Google LLC
use async_trait::async_trait;
use com_sdv_google_user_preferences_view_change_notifier_rpc::Interface as ViewChangeNotifier;
use user_preferences_api::change_notifier::{OnSettingsChangeRequest, OnSettingsChangeResponse};
use user_preferences_utils::print_settings;
pub struct HmiServiceImpl {}

impl HmiServiceImpl {
    pub fn new() -> Self {
        Self {}
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
            println!(
                "The following settings were changed in settings group {}/{}",
                request.settings_group_id.service_fqin, request.settings_group_id.name
            );
            print_settings(&request.active_settings);
        }

        Ok(OnSettingsChangeResponse::new())
    }
}
