use sdvgenerated::user_preferences_view_interface::UserPreferencesView;
use user_preferences_api::user_preferences_view::{
    OnSettingsChangeRequest, OnSettingsChangeResponse,
};
use user_preferences_utils::print_settings;

pub(crate) struct HmiServiceImpl {}

impl HmiServiceImpl {
    pub fn new() -> Self {
        Self {}
    }
}

impl UserPreferencesView for HmiServiceImpl {
    fn on_settings_change(
        &self,
        request: OnSettingsChangeRequest,
    ) -> sdvmiddleware_rpc::RpcResult<OnSettingsChangeResponse> {
        if request.group.is_some() {
            println!("Received settings change notification {}", request.group.name);
            print_settings(&request.group.settings);
        }

        Ok(OnSettingsChangeResponse::new())
    }
}
