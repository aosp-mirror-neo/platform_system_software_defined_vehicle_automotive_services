// Copyright 2023 Google LLC

use crate::binder::SpIBinder;
use crate::vpm_system_service::VPMSystemService;
use android_sdv_vpm_aidl::aidl::android::sdv::vpm::IVpmSystemApi::{BnVpmSystemApi, IVpmSystemApi};
use android_sdv_vpm_aidl::aidl::android::sdv::vpm::PowerState::PowerState;
use android_sdv_vpm_aidl::binder;
use log::info;
use tracing::instrument;

pub struct VPMSystemServiceApi<'a> {
    vpm_system_service: &'a VPMSystemService,
}

impl<'a> VPMSystemServiceApi<'a> {
    pub fn new(vpm_system_service: &'a VPMSystemService) -> Self {
        Self { vpm_system_service }
    }
}

impl binder::Interface for VPMSystemServiceApi<'static> {}

impl IVpmSystemApi for VPMSystemServiceApi<'static> {
    #[instrument(skip(self))]
    fn setPowerState(&self, in_power_state: PowerState) -> binder::Result<PowerState> {
        info!("Updating vehicle state to new power state {:?}", in_power_state);
        Ok(self.vpm_system_service.set_power_state(in_power_state))
    }

    #[instrument(skip(self))]
    fn setCustomState(
        &self,
        state_machine_name: &str,
        state_machine_value: &str,
    ) -> binder::Result<String> {
        info!(
            "Updating vehicle custom state {:?} to value {:?}",
            state_machine_name, state_machine_value
        );
        Ok(self
            .vpm_system_service
            .set_custom_state(state_machine_name.to_string(), state_machine_value.to_string()))
    }
}

#[instrument(skip(vpm_system_service))]
pub(crate) fn create_system_binder(
    vpm_system_service: &'static VPMSystemService,
) -> anyhow::Result<SpIBinder> {
    Ok(BnVpmSystemApi::new_binder(
        VPMSystemServiceApi::new(vpm_system_service),
        binder::BinderFeatures::default(),
    )
    .as_binder())
}
