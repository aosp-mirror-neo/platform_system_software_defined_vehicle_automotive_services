// Copyright 2024 Google LLC

//! SDV ConCal Service Bundle Sample Implementation.

use concal_api_stable::concal_registration_service::*;
use concal_api_stable::concal_update_service::*;

use concal_api_stable::config_override::ConfigOverride;
use concal_api_stable::configuration::ConfigMetadata;
use protobuf::Message;
use protobuf::MessageField;
use rand::Rng;
use sdv_status_rs::SdvStatusCode;

use std::collections::HashMap;
use std::hash::Hash;
use std::option::Option;
use std::sync::Mutex;

use crate::configuration::*;
use crate::hashable_config_id::HashableConfigId;
use crate::trie::*;
use crate::versioned_object::VersionedObject;

macro_rules! acquire_lock_or_return_err {
    ($mutex:expr,$return_type:ident) => {{
        match $mutex.lock() {
            Ok(x) => x,
            Err(e) => {
                return Err(sdv_status_rs::SdvStatus::with_message(
                    SdvStatusCode::Unavailable,
                    format!("Unable to obtain mutex: {:?}", e),
                ))
            }
        }
    }};
}

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
struct OverrideKey {
    config_id: HashableConfigId,
    override_id: String,
}

struct ConCalServiceData {
    base_configs: HashMap<HashableConfigId, Configuration>,
    derived_configs: VersionedObject<HashMap<HashableConfigId, Configuration>>,
    overrides: VersionedObject<HashMap<OverrideKey, ConfigOverride>>,
    current_update_id: Option<i64>,
}

impl ConCalServiceData {
    fn new() -> Self {
        ConCalServiceData {
            base_configs: HashMap::new(),
            // We need 3 versions of the hashmap since during an update we need to have the following:
            // 1. A stable configuration to rollback too
            // 2. The Current Configuration
            // 3. A hashmap to use for computing the new configuration while keeping 1 & 2 since updates can be cancelled or encounter failures
            derived_configs: VersionedObject::new(HashMap::new(), 3),
            overrides: VersionedObject::new(HashMap::new(), 3),
            current_update_id: None,
        }
    }
}

pub(crate) struct ConCalServiceBundleImpl {
    data: Mutex<ConCalServiceData>,
}

impl ConCalServiceBundleImpl {
    pub fn new() -> Self {
        ConCalServiceBundleImpl { data: Mutex::new(ConCalServiceData::new()) }
    }

    fn group_overrides_by_config_id(
        &self,
        mlock: &std::sync::MutexGuard<'_, ConCalServiceData>,
    ) -> HashMap<HashableConfigId, Vec<ConfigOverride>> {
        let mut trie: Trie<HashableConfigId> = Trie::new();
        mlock.base_configs.keys().for_each(|x| {
            trie.add(x.fields(), x.clone());
        });

        let mut result: HashMap<HashableConfigId, Vec<ConfigOverride>> = HashMap::new();

        for (key, value) in mlock.overrides.iter() {
            let matching_config_ids = trie.get(&key.config_id.fields());
            for config_id in matching_config_ids.iter() {
                result
                    .entry(config_id.clone())
                    .and_modify(|v| v.push(value.clone()))
                    .or_insert(vec![value.clone()]);
            }
        }

        result
    }

    fn calculate_snapshot(
        &self,
        base_configs: &HashMap<HashableConfigId, Configuration>,
        overrides_for_each_config: &HashMap<HashableConfigId, Vec<ConfigOverride>>,
    ) -> Result<HashMap<HashableConfigId, Configuration>, String> {
        let mut new_configurations: HashMap<HashableConfigId, Configuration> = HashMap::new();
        for (config_id, base_config) in base_configs.iter() {
            let overrides = overrides_for_each_config.get(config_id);

            if let Some(overrides) = overrides {
                let updated_config = base_config.with_overrides(overrides)?;
                new_configurations.insert(config_id.clone(), updated_config);
            } else {
                new_configurations.insert(config_id.clone(), base_config.clone());
            }
        }

        Ok(new_configurations)
    }
}

#[async_trait::async_trait]
impl sdv_concal_con_cal_registration_service_rpc::Interface for ConCalServiceBundleImpl {
    async fn GetConfig(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        request: &concal_api_stable::concal_registration_service::GetConfigRequest,
    ) -> sdv_status_rs::SdvResult<concal_api_stable::concal_registration_service::GetConfigResponse>
    {
        let mlock = acquire_lock_or_return_err!(self.data, GetConfigResponse);

        if request.config_id.is_none() {
            return Err(sdv_status_rs::SdvStatus::with_message(
                SdvStatusCode::InvalidArgument,
                "Request does not provide a config id",
            ));
        }

        let config_id = HashableConfigId::from(request.config_id.clone().unwrap());
        let config =
            match mlock.derived_configs.get(&config_id).or(mlock.base_configs.get(&config_id)) {
                Some(x) => x,
                None => {
                    return Err(sdv_status_rs::SdvStatus::with_message(
                        SdvStatusCode::NotFound,
                        format!("Config with ID {:?} does not exist", config_id),
                    ))
                }
            };

        let config_bytes = config.data().clone().write_to_bytes().unwrap();
        return Ok(GetConfigResponse {
            config: config_bytes,
            config_version: config.version().clone(),
            ..Default::default()
        });
    }

    async fn RegisterConfigMetadata(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        request: &concal_api_stable::concal_registration_service::RegisterConfigMetadataRequest,
    ) -> sdv_status_rs::SdvResult<
        concal_api_stable::concal_registration_service::RegisterConfigMetadataResponse,
    > {
        let mut mlock = acquire_lock_or_return_err!(self.data, RegisterConfigMetadataResponse);

        let config_id = match request.config_id.is_some() {
            true => HashableConfigId::from(request.config_id.clone().unwrap()),
            false => {
                return Err(sdv_status_rs::SdvStatus::with_message(
                    SdvStatusCode::InvalidArgument,
                    "ConfigId is empty",
                ));
            }
        };
        if config_id.contains_empty_fields() {
            return Err(sdv_status_rs::SdvStatus::with_message(
                SdvStatusCode::InvalidArgument,
                format!("ConfigId fields contain an empty string: {:?}", config_id),
            ));
        }
        if mlock.base_configs.contains_key(&config_id)
            && mlock.base_configs.get(&config_id).unwrap().version() == &request.config_version
        {
            return Ok(RegisterConfigMetadataResponse { ..Default::default() });
        }

        let metadata: ConfigMetadata = match request.metadata.is_some() {
            true => request.metadata.clone().unwrap(),
            false => {
                return Err(sdv_status_rs::SdvStatus::with_message(
                    SdvStatusCode::InvalidArgument,
                    format!("Metadata for config {:?} is not initialized", config_id),
                ));
            }
        };

        if metadata.default_config.is_none() {
            return Err(sdv_status_rs::SdvStatus::with_message(
                SdvStatusCode::InvalidArgument,
                format!("Default configuration for config {:?} is not provided", config_id),
            ));
        }

        if metadata.descriptor_set.is_none() {
            return Err(sdv_status_rs::SdvStatus::with_message(
                SdvStatusCode::InvalidArgument,
                format!("Descriptor set for config {:?} is not provided", config_id),
            ));
        }

        let config = Configuration::new(
            metadata.default_config.unwrap().clone(),
            metadata.descriptor_set.unwrap().clone(),
            request.config_version.clone(),
        );

        mlock.base_configs.insert(config_id, config);

        Ok(RegisterConfigMetadataResponse { ..Default::default() })
    }

    async fn GetConfigMetadata(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        request: &concal_api_stable::concal_registration_service::GetConfigMetadataRequest,
    ) -> sdv_status_rs::SdvResult<
        concal_api_stable::concal_registration_service::GetConfigMetadataResponse,
    > {
        let mlock = acquire_lock_or_return_err!(self.data, GetConfigMetadataResponse);

        let config_id = match request.config_id.is_some() {
            true => HashableConfigId::from(request.config_id.clone().unwrap()),
            false => {
                return Err(sdv_status_rs::SdvStatus::with_message(
                    SdvStatusCode::InvalidArgument,
                    "ConfigId is empty",
                ));
            }
        };
        if config_id.contains_empty_fields() {
            return Err(sdv_status_rs::SdvStatus::with_message(
                SdvStatusCode::InvalidArgument,
                format!("ConfigId fields contain an empty string: {:?}", config_id),
            ));
        }

        let config = match mlock.base_configs.get(&config_id) {
            Some(x) => x,
            None => return Ok(GetConfigMetadataResponse::new()),
        };

        let mut response = GetConfigMetadataResponse::new();
        response.metadata = MessageField::some(ConfigMetadata {
            default_config: MessageField::some(config.data().clone()),
            descriptor_set: MessageField::some(config.descriptor().clone()),
            ..Default::default()
        });

        Ok(response)
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
        let mut mlock = acquire_lock_or_return_err!(self.data, StartConfigUpdateResponse);
        mlock.current_update_id = match mlock.current_update_id {
            Some(id) => {
                return Err(sdv_status_rs::SdvStatus::with_message(
                    SdvStatusCode::FailedPrecondition,
                    format!("Update with ID {:?} already in progress", id),
                ));
            }
            None => Some(rand::thread_rng().gen::<i64>()),
        };

        mlock.overrides.snapshot();

        Ok(StartConfigUpdateResponse {
            update_id: mlock.current_update_id.unwrap(),
            ..Default::default()
        })
    }
    async fn AddOrUpdateConfigOverride(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        request: &concal_api_stable::concal_update_service::AddOrUpdateConfigOverrideRequest,
    ) -> sdv_status_rs::SdvResult<
        concal_api_stable::concal_update_service::AddOrUpdateConfigOverrideResponse,
    > {
        let mut mlock = acquire_lock_or_return_err!(self.data, AddOrUpdateConfigOverrideResponse);

        for config_override in request.configOverride.iter() {
            if config_override.config_id.is_none() {
                return Err(sdv_status_rs::SdvStatus::with_message(
                    SdvStatusCode::InvalidArgument,
                    "ConfigId is empty",
                ));
            }
        }

        if mlock.current_update_id.is_none()
            || request.update_id != mlock.current_update_id.unwrap()
        {
            return Err(sdv_status_rs::SdvStatus::with_message(
                SdvStatusCode::InvalidArgument,
                format!(
                    "Invalid update request, current update has id {:?}, request has id {}",
                    mlock.current_update_id, request.update_id,
                ),
            ));
        }

        for config_override in request.configOverride.iter() {
            mlock.overrides.insert(
                OverrideKey {
                    config_id: HashableConfigId::from(config_override.config_id.clone().unwrap()),
                    override_id: config_override.override_id.clone(),
                },
                config_override.clone(),
            );
        }

        Ok(AddOrUpdateConfigOverrideResponse { ..Default::default() })
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
        request: &concal_api_stable::concal_update_service::CancelConfigUpdateRequest,
    ) -> sdv_status_rs::SdvResult<
        concal_api_stable::concal_update_service::CancelConfigUpdateResponse,
    > {
        let mut mlock = acquire_lock_or_return_err!(self.data, CancelConfigUpdateResponse);

        if mlock.current_update_id.is_none()
            || request.update_id != mlock.current_update_id.unwrap()
        {
            return Err(sdv_status_rs::SdvStatus::with_message(
                SdvStatusCode::InvalidArgument,
                format!(
                    "Invalid update request, current update has id {:?}, request has id {}",
                    mlock.current_update_id, request.update_id,
                ),
            ));
        }

        mlock.current_update_id = None;
        mlock.overrides.rollback();

        Ok(CancelConfigUpdateResponse { ..Default::default() })
    }
    async fn FinishConfigUpdate(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        request: &concal_api_stable::concal_update_service::FinishConfigUpdateRequest,
    ) -> sdv_status_rs::SdvResult<
        concal_api_stable::concal_update_service::FinishConfigUpdateResponse,
    > {
        let mut mlock = acquire_lock_or_return_err!(self.data, FinishConfigUpdateResponse);

        if mlock.current_update_id.is_none()
            || request.update_id != mlock.current_update_id.unwrap()
        {
            return Err(sdv_status_rs::SdvStatus::with_message(
                SdvStatusCode::InvalidArgument,
                format!(
                    "Current update has ID {:?} and requested update has id {:?}",
                    mlock.current_update_id, request.update_id,
                ),
            ));
        }

        mlock.current_update_id = None;

        let overrides_for_each_config: HashMap<HashableConfigId, Vec<ConfigOverride>> =
            self.group_overrides_by_config_id(&mlock);
        let new_configurations =
            match self.calculate_snapshot(&mlock.base_configs, &overrides_for_each_config) {
                Ok(x) => x,
                Err(err) => {
                    return Err(sdv_status_rs::SdvStatus::with_message(
                        SdvStatusCode::Internal,
                        format!("Update {:?} failed due to {:?}", mlock.current_update_id, err),
                    ));
                }
            };

        // Save existing configuration.
        mlock.derived_configs.snapshot();
        *mlock.derived_configs = new_configurations;

        Ok(FinishConfigUpdateResponse { ..Default::default() })
    }
    async fn Rollback(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        _request: &concal_api_stable::concal_update_service::RollbackRequest,
    ) -> sdv_status_rs::SdvResult<concal_api_stable::concal_update_service::RollbackResponse> {
        let mut mlock = acquire_lock_or_return_err!(self.data, RollbackResponse);

        if mlock.current_update_id.is_some() {
            return Err(sdv_status_rs::SdvStatus::with_message(
                SdvStatusCode::InvalidArgument,
                format!(
                    "Cannot rollback while an update {:?} is in progress",
                    mlock.current_update_id
                ),
            ));
        }

        mlock.overrides.rollback();
        mlock.derived_configs.rollback();

        Ok(RollbackResponse { ..Default::default() })
    }
    async fn Reset(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        _request: &concal_api_stable::concal_update_service::ResetRequest,
    ) -> sdv_status_rs::SdvResult<concal_api_stable::concal_update_service::ResetResponse> {
        let mut mlock = acquire_lock_or_return_err!(self.data, ResetResponse);

        if mlock.current_update_id.is_some() {
            return Err(sdv_status_rs::SdvStatus::with_message(
                SdvStatusCode::FailedPrecondition,
                format!(
                    "Cannot perform a reset while an update {:?} is in progress",
                    mlock.current_update_id,
                ),
            ));
        }

        *mlock = ConCalServiceData::new();

        Ok(ResetResponse { ..Default::default() })
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

#[cfg(test)]
mod tests {

    use super::*;
    use concal_api_stable::config_override::config_override_key_value::Value;
    use concal_api_stable::config_override::ConfigOverrideKeyValue;
    use concal_api_stable::configuration::ConfigId;
    use protobuf::well_known_types::any::Any;
    use sdv_comms::id::ServiceFqin;
    use sdv_concal_con_cal_registration_service_rpc::Interface as OtherInterface;
    use sdv_concal_con_cal_update_service_rpc::Interface;
    use sdv_status_rs::SdvStatusCode;
    use sdvsampleconcal_proto_rust::test_configuration::TestConfiguration;
    use std::str::FromStr;

    use crate::tests::utilities::*;

    #[tokio::test]
    async fn get_config_returns_error_when_request_contains_none_config_id() {
        let service = ConCalServiceBundleImpl::new();

        let result = service
            .GetConfig(
                ServiceFqin::from_str(
                    "vm:sdv.google.sample.concal.ConCalSampleServiceBundle/default",
                )
                .unwrap(),
                &GetConfigRequest { ..Default::default() },
            )
            .await;

        assert_eq!(result.unwrap_err().status_code(), SdvStatusCode::InvalidArgument);
    }

    #[tokio::test]
    async fn get_config_returns_error_when_config_doesnt_exist() {
        let config_id = test_config_id();

        let service = ConCalServiceBundleImpl::new();

        let result = service
            .GetConfig(
                ServiceFqin::from_str(
                    "vm:sdv.google.sample.concal.ConCalSampleServiceBundle/default",
                )
                .unwrap(),
                &GetConfigRequest {
                    config_id: MessageField::some(config_id),
                    ..Default::default()
                },
            )
            .await;
        assert_eq!(result.unwrap_err().status_code(), SdvStatusCode::NotFound);
    }

    #[tokio::test]
    async fn get_config_can_return_default_config() {
        let config = test_config();
        let config_id = test_config_id();

        let service = ConCalServiceBundleImpl::new();

        let resp = register_config_metadata(&service, &config_id, &config).await;
        assert!(resp.is_ok());

        let resp = get_config(&service, &config_id).await;
        assert!(resp.is_ok());

        let resp = resp.unwrap();
        assert_eq!(resp.clone().config_version.as_str(), config.version().as_str());
        assert_eq!(resp.config, config.data().write_to_bytes().unwrap());
    }

    #[tokio::test]
    async fn calculates_derived_config_correctly() {
        let service = ConCalServiceBundleImpl::new();
        let config = test_config();
        let config_id = test_config_id();

        let resp = register_config_metadata(&service, &config_id, &config).await;
        assert!(resp.is_ok());

        let overrides = create_sample_overrides(
            &config_id,
            String::from("\"1\""),
            true,
            String::from("\"new_value\""),
        );

        let resp = start_config_update(&service).await;
        assert!(resp.is_ok());

        let update_id = resp.unwrap().update_id;

        let resp = add_or_update_config_override(&service, update_id, &overrides).await;
        assert!(resp.is_ok());

        let resp = finish_config_update(&service, update_id).await;
        assert!(resp.is_ok());

        let resp = get_config(&service, &config_id).await;
        assert!(resp.is_ok());
        let resp = resp.unwrap();
        assert_eq!(resp.clone().config_version.as_str(), config.version().as_str());

        let any = Any::parse_from_bytes(&resp.config).expect("to convert bytes to any");
        let derived_config = TestConfiguration::parse_from_bytes(&any.value)
            .expect("to unmarshal TestConfiguration from bytes");

        assert!(derived_config.bool_parameter);
        assert_eq!(derived_config.str_parameter, String::from("new_value"));
    }

    #[tokio::test]
    async fn can_rollback_to_previous_config() {
        let service = ConCalServiceBundleImpl::new();
        let config = test_config();
        let config_id = test_config_id();

        let resp = register_config_metadata(&service, &config_id, &config).await;
        assert!(resp.is_ok());

        let overrides = create_sample_overrides(
            &config_id,
            String::from("\"1\""),
            true,
            String::from("\"new_value\""),
        );

        let resp = start_config_update(&service).await;
        assert!(resp.is_ok());

        let update_id = resp.unwrap().update_id;

        let resp = add_or_update_config_override(&service, update_id, &overrides).await;
        assert!(resp.is_ok());

        let resp = finish_config_update(&service, update_id).await;
        assert!(resp.is_ok());

        let resp = service
            .Rollback(
                ServiceFqin::from_str(
                    "vm:sdv.google.sample.concal.ConCalSampleServiceBundle/default",
                )
                .unwrap(),
                &RollbackRequest::new(),
            )
            .await;
        assert!(resp.is_ok());

        let resp = get_config(&service, &config_id).await;
        assert!(resp.is_ok());
        let resp = resp.unwrap();

        assert_eq!(resp.clone().config_version.as_str(), config.version().as_str());
        assert_eq!(resp.config, config.data().write_to_bytes().unwrap());
    }

    #[tokio::test]
    async fn reset_fails_during_updates() {
        let service = ConCalServiceBundleImpl::new();
        let config = test_config();
        let config_id = test_config_id();

        let resp = register_config_metadata(&service, &config_id, &config).await;
        assert!(resp.is_ok());

        let resp = start_config_update(&service).await;
        assert!(resp.is_ok());

        let resp = service
            .Reset(
                ServiceFqin::from_str(
                    "vm:sdv.google.sample.concal.ConCalSampleServiceBundle/default",
                )
                .unwrap(),
                &ResetRequest::new(),
            )
            .await;
        assert_eq!(resp.unwrap_err().status_code(), SdvStatusCode::FailedPrecondition);
    }

    #[tokio::test]
    async fn reset_removes_all_configs_and_overrides_when_successful() {
        let service = ConCalServiceBundleImpl::new();
        let config = test_config();
        let config_id = test_config_id();

        // Register a config with overrides and reset afterwards to ensure all changes are deleted
        let resp = register_config_metadata(&service, &config_id, &config).await;
        assert!(resp.is_ok());

        let overrides = create_sample_overrides(
            &config_id,
            String::from("\"1\""),
            true,
            String::from("\"new_value\""),
        );

        let resp = start_config_update(&service).await;
        assert!(resp.is_ok());

        let update_id = resp.unwrap().update_id;

        let resp = add_or_update_config_override(&service, update_id, &overrides).await;
        assert!(resp.is_ok());

        let resp = finish_config_update(&service, update_id).await;
        assert!(resp.is_ok());

        let resp = service
            .Reset(
                ServiceFqin::from_str(
                    "vm:sdv.google.sample.concal.ConCalSampleServiceBundle/default",
                )
                .unwrap(),
                &ResetRequest::new(),
            )
            .await;
        assert!(resp.is_ok());

        let resp = get_config(&service, &config_id).await;
        assert_eq!(resp.unwrap_err().status_code(), SdvStatusCode::NotFound);

        let resp = register_config_metadata(&service, &config_id, &config).await;
        assert!(resp.is_ok());

        //Ensure that any update doesn't apply overrides that were created before the reset.
        let resp = start_config_update(&service).await;
        assert!(resp.is_ok());

        let resp = finish_config_update(&service, resp.unwrap().update_id).await;
        assert!(resp.is_ok());

        let resp = get_config(&service, &config_id).await;
        assert!(resp.is_ok());

        let resp = resp.unwrap();
        assert_eq!(resp.clone().config_version.as_str(), config.version().as_str());
        assert_eq!(resp.config, config.data().write_to_bytes().unwrap());
    }

    async fn start_config_update(
        service: &ConCalServiceBundleImpl,
    ) -> sdv_status_rs::SdvResult<concal_api_stable::concal_update_service::StartConfigUpdateResponse>
    {
        service
            .StartConfigUpdate(
                ServiceFqin::from_str(
                    "vm:sdv.google.sample.concal.ConCalSampleServiceBundle/default",
                )
                .unwrap(),
                &StartConfigUpdateRequest { ..Default::default() },
            )
            .await
    }

    async fn add_or_update_config_override(
        service: &ConCalServiceBundleImpl,
        update_id: i64,
        overrides: &[ConfigOverride],
    ) -> sdv_status_rs::SdvResult<
        concal_api_stable::concal_update_service::AddOrUpdateConfigOverrideResponse,
    > {
        service
            .AddOrUpdateConfigOverride(
                ServiceFqin::from_str(
                    "vm:sdv.google.sample.concal.ConCalSampleServiceBundle/default",
                )
                .unwrap(),
                &AddOrUpdateConfigOverrideRequest {
                    update_id,
                    configOverride: overrides.to_vec(),
                    ..Default::default()
                },
            )
            .await
    }

    async fn finish_config_update(
        service: &ConCalServiceBundleImpl,
        update_id: i64,
    ) -> sdv_status_rs::SdvResult<
        concal_api_stable::concal_update_service::FinishConfigUpdateResponse,
    > {
        service
            .FinishConfigUpdate(
                ServiceFqin::from_str(
                    "vm:sdv.google.sample.concal.ConCalSampleServiceBundle/default",
                )
                .unwrap(),
                &FinishConfigUpdateRequest { update_id, ..Default::default() },
            )
            .await
    }

    async fn get_config(
        service: &ConCalServiceBundleImpl,
        config_id: &ConfigId,
    ) -> sdv_status_rs::SdvResult<concal_api_stable::concal_registration_service::GetConfigResponse>
    {
        service
            .GetConfig(
                ServiceFqin::from_str(
                    "vm:sdv.google.sample.concal.ConCalSampleServiceBundle/default",
                )
                .unwrap(),
                &GetConfigRequest {
                    config_id: MessageField::some(config_id.clone()),
                    ..Default::default()
                },
            )
            .await
    }

    fn create_sample_overrides(
        config_id: &ConfigId,
        override_id: String,
        bool_parameter: bool,
        str_parameter: String,
    ) -> Vec<ConfigOverride> {
        vec![ConfigOverride {
            config_id: MessageField::some(config_id.clone()),
            override_id,
            pairs: vec![
                ConfigOverrideKeyValue {
                    key: String::from("bool_parameter"),
                    value: Option::Some(Value::ValueTxtproto(bool_parameter.to_string())),
                    ..Default::default()
                },
                ConfigOverrideKeyValue {
                    key: String::from("str_parameter"),
                    value: Option::Some(Value::ValueTxtproto(str_parameter)),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }]
    }

    async fn register_config_metadata(
        service: &ConCalServiceBundleImpl,
        id: &ConfigId,
        config: &Configuration,
    ) -> sdv_status_rs::SdvResult<
        concal_api_stable::concal_registration_service::RegisterConfigMetadataResponse,
    > {
        service
            .RegisterConfigMetadata(
                ServiceFqin::from_str(
                    "vm:sdv.google.sample.concal.ConCalSampleServiceBundle/default",
                )
                .unwrap(),
                &RegisterConfigMetadataRequest {
                    config_id: MessageField::some(id.clone()),
                    metadata: MessageField::some(ConfigMetadata {
                        descriptor_set: MessageField::some(config.descriptor().clone()),
                        default_config: MessageField::some(config.data().clone()),
                        ..Default::default()
                    }),
                    config_version: config.version().clone(),
                    ..Default::default()
                },
            )
            .await
    }
}
