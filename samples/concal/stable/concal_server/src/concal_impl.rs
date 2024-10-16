// Copyright 2024 Google LLC

//! SDV ConCal Service Bundle Sample Implementation.

pub(crate) struct ConCalServiceBundleImpl {}

#[async_trait::async_trait]
impl sdv_concal_con_cal_registration_service_rpc::Interface for ConCalServiceBundleImpl {
    async fn GetConfig(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        _request: &concal_api_stable::concal_registration_service::GetConfigRequest,
    ) -> sdv_status_rs::SdvResult<concal_api_stable::concal_registration_service::GetConfigResponse>
    {
        todo!();
    }
    async fn RegisterConfigMetadata(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        _request: &concal_api_stable::concal_registration_service::RegisterConfigMetadataRequest,
    ) -> sdv_status_rs::SdvResult<
        concal_api_stable::concal_registration_service::RegisterConfigMetadataResponse,
    > {
        todo!();
    }
    async fn GetConfigMetadata(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        _request: &concal_api_stable::concal_registration_service::GetConfigMetadataRequest,
    ) -> sdv_status_rs::SdvResult<
        concal_api_stable::concal_registration_service::GetConfigMetadataResponse,
    > {
        todo!();
    }
}

#[async_trait::async_trait]
impl sdv_concal_con_cal_update_service_rpc::Interface for ConCalServiceBundleImpl {
    async fn StartConfigUpdate(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        _request: &concal_api_stable::concal_update_service::StartConfigUpdateRequest,
    ) -> sdv_status_rs::SdvResult<concal_api_stable::concal_update_service::StartConfigUpdateResponse>
    {
        todo!();
    }
    async fn AddOrUpdateConfigOverride(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        _request: &concal_api_stable::concal_update_service::AddOrUpdateConfigOverrideRequest,
    ) -> sdv_status_rs::SdvResult<
        concal_api_stable::concal_update_service::AddOrUpdateConfigOverrideResponse,
    > {
        todo!();
    }
    async fn RemoveConfigOverride(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        _request: &concal_api_stable::concal_update_service::RemoveConfigOverrideRequest,
    ) -> sdv_status_rs::SdvResult<
        concal_api_stable::concal_update_service::RemoveConfigOverrideResponse,
    > {
        todo!();
    }
    async fn CancelConfigUpdate(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        _request: &concal_api_stable::concal_update_service::CancelConfigUpdateRequest,
    ) -> sdv_status_rs::SdvResult<
        concal_api_stable::concal_update_service::CancelConfigUpdateResponse,
    > {
        todo!();
    }
    async fn FinishConfigUpdate(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        _request: &concal_api_stable::concal_update_service::FinishConfigUpdateRequest,
    ) -> sdv_status_rs::SdvResult<
        concal_api_stable::concal_update_service::FinishConfigUpdateResponse,
    > {
        todo!();
    }
    async fn Rollback(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        _request: &concal_api_stable::concal_update_service::RollbackRequest,
    ) -> sdv_status_rs::SdvResult<concal_api_stable::concal_update_service::RollbackResponse> {
        todo!();
    }
    async fn Reset(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        _request: &concal_api_stable::concal_update_service::ResetRequest,
    ) -> sdv_status_rs::SdvResult<concal_api_stable::concal_update_service::ResetResponse> {
        todo!();
    }
    async fn UninstallConfig(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        _request: &concal_api_stable::concal_update_service::UninstallConfigRequest,
    ) -> sdv_status_rs::SdvResult<concal_api_stable::concal_update_service::UninstallConfigResponse>
    {
        todo!();
    }
}

#[async_trait::async_trait]
impl sdv_concal_calib_con_cal_calibration_service_rpc::Interface for ConCalServiceBundleImpl {
    async fn GetCalibrationConfigIds(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        _request: &concal_api_stable::concal_calibration_service::GetCalibrationConfigIdsRequest,
    ) -> sdv_status_rs::SdvResult<
        concal_api_stable::concal_calibration_service::GetCalibrationConfigIdsResponse,
    > {
        todo!();
    }
    async fn StartCalibration(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        _request: &concal_api_stable::calibratable_service::StartCalibrationRequest,
    ) -> sdv_status_rs::SdvResult<concal_api_stable::calibratable_service::StartCalibrationResponse>
    {
        todo!();
    }
    async fn UpdateConfigForCalibration(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        _request: &concal_api_stable::concal_calibration_service::UpdateConfigForCalibrationRequest,
    ) -> sdv_status_rs::SdvResult<
        concal_api_stable::concal_calibration_service::UpdateConfigForCalibrationResponse,
    > {
        todo!();
    }
    async fn FinishCalibration(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        _request: &concal_api_stable::calibratable_service::FinishCalibrationRequest,
    ) -> sdv_status_rs::SdvResult<concal_api_stable::calibratable_service::FinishCalibrationResponse>
    {
        todo!();
    }
}

#[async_trait::async_trait]
impl sdv_concal_calib_con_cal_calibration_notification_service_rpc::Interface
    for ConCalServiceBundleImpl
{
    async fn RegisterForCalibrationUpdate(&self,
        _caller_id: sdv_comms::id::ServiceFqin,
        _request: &concal_api_stable::concal_calibration_notification_service::RegisterForCalibrationUpdateRequest)
    -> sdv_status_rs::SdvResult<concal_api_stable::concal_calibration_notification_service::RegisterForCalibrationUpdateResponse>{
        todo!();
    }
    async fn UnregisterFromCalibrationUpdate(&self,
        _caller_id: sdv_comms::id::ServiceFqin,
        _request: &concal_api_stable::concal_calibration_notification_service::UnregisterFromCalibrationUpdateRequest)
    -> sdv_status_rs::SdvResult<concal_api_stable::concal_calibration_notification_service::UnregisterFromCalibrationUpdateResponse>{
        todo!();
    }
}
