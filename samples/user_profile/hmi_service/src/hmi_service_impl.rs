use sdvgenerated::user_profile_view_interface::UserProfileView;
use user_profile_api::user_profile_view::{OnSettingsChangeRequest, OnSettingsChangeResponse};
use user_profile_utils::print_settings;

pub(crate) struct HmiServiceImpl {}

impl HmiServiceImpl {
    pub fn new() -> Self {
        Self {}
    }
}

impl UserProfileView for HmiServiceImpl {
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
