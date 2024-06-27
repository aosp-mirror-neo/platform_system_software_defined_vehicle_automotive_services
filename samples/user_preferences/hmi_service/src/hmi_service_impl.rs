// Copyright 2024 Google LLC

use sdvgenerated_rpc_sdv_user_preferences_view_change_notifier::interface::ChangeNotifier;
use user_preferences_api::change_notifier::{OnSettingsChangeRequest, OnSettingsChangeResponse};
use user_preferences_utils::print_settings;

pub(crate) struct HmiServiceImpl {}

impl HmiServiceImpl {
    pub fn new() -> Self {
        Self {}
    }
}

impl ChangeNotifier for HmiServiceImpl {
    fn on_settings_change(
        &self,
        request: OnSettingsChangeRequest,
    ) -> sdvmiddleware_rpc::RpcResult<OnSettingsChangeResponse> {
        if request.settings_group_id.is_some() {
            println!(
                "The following settings were changed in settings group {}/{}",
                request.settings_group_id.service_fqin, request.settings_group_id.name
            );
            print_settings(&request.settings);
        }

        Ok(OnSettingsChangeResponse::new())
    }
}
