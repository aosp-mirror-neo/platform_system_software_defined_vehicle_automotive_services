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
        if request.settings_group.is_some() {
            println!(
                "The following settings were changed in settings group {}/{}",
                request.settings_group.id.service_fqin, request.settings_group.id.name
            );
            print_settings(&request.settings_group.settings);
        }
        print_settings(&request.settings_group.settings);

        Ok(OnSettingsChangeResponse::new())
    }
}
