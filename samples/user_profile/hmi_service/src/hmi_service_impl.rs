use sdvgenerated::user_profile_view_interface::UserProfileView;
use user_profile_api::user_profile_view::{OnConfigChangeRequest, OnConfigChangeResponse};
use user_profile_utils::print_settings;

pub(crate) struct HmiServiceImpl {}

impl HmiServiceImpl {
    pub fn new() -> Self {
        Self {}
    }
}

impl UserProfileView for HmiServiceImpl {
    fn on_config_change(
        &self,
        request: OnConfigChangeRequest,
    ) -> sdvmiddleware_rpc::RpcResult<OnConfigChangeResponse> {
        if request.config.is_some() {
            println!("Received config change notification {}", request.config.config_name);
            print_settings(&request.config.settings);
        }

        Ok(OnConfigChangeResponse::new())
    }
}
