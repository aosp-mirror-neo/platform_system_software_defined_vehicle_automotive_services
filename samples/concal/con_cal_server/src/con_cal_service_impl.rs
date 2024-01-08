use concal_api::calibration::*;
use concal_api::concal_service::*;
use concal_api::configuration::ConfigMetadata;
use concal_api::configuration::ConfigOverride;
use concal_api::error::{Error, ResponseCode};
use protobuf::EnumOrUnknown;
use protobuf::Message;
use protobuf::MessageField;
use rand::Rng;
use sdvgenerated::con_cal_service_interface::ConCalService;
use sdvmiddleware_rpc::RpcResult;
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
                return Ok($return_type {
                    error: ConCalServiceImpl::error(
                        ResponseCode::RUNTIME_ERROR,
                        format!("Unable to obtain mutex: {:?}", e),
                    ),
                    ..Default::default()
                })
            }
        }
    }};
}

macro_rules! error_response {
    ($return_type:ident, $response_code:expr, $error_message:literal, $($error_message_format_arg: tt)*) => {{
        Ok($return_type {
            error: ConCalServiceImpl::error(
                $response_code,
                format!($error_message, $($error_message_format_arg)*),
            ),
            ..Default::default()
        })
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

pub struct ConCalServiceImpl {
    data: Mutex<ConCalServiceData>,
}

impl ConCalServiceImpl {
    pub fn new() -> Self {
        ConCalServiceImpl {
            data: Mutex::new(ConCalServiceData {
                base_configs: HashMap::new(),
                // We would like to keep 3 versions of the hashmap stored in versionedObject.
                // we need 3 since during an update we can have the following
                // { Configuration to rollback too, Current Configuration, New configuration being computed which might fail
                // and therefore we should remain at the current config and keep the older version to be able to rollback }.
                // same logic for overrides since we can cancel an ongoing update
                // and therefore need to discard all overrides added during this update
                derived_configs: VersionedObject::new(HashMap::new(), 3),
                overrides: VersionedObject::new(HashMap::new(), 3),
                current_update_id: None,
            }),
        }
    }

    fn error(error_code: ResponseCode, message: String) -> MessageField<Error> {
        MessageField::some(Error {
            error_code: EnumOrUnknown::from(error_code),
            message,
            ..Default::default()
        })
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

impl ConCalService for ConCalServiceImpl {
    /// RPC method declared in the interface.
    fn get_config(&self, request: GetConfigRequest) -> RpcResult<GetConfigResponse> {
        let mlock = acquire_lock_or_return_err!(self.data, GetConfigResponse);

        if request.config_id.is_none() {
            return error_response!(
                GetConfigResponse,
                ResponseCode::INVALID_REQUEST,
                "Request does not provide a config id",
            );
        }

        let config_id = HashableConfigId::from(request.config_id.unwrap());
        let config =
            match mlock.derived_configs.get(&config_id).or(mlock.base_configs.get(&config_id)) {
                Some(x) => x,
                None => {
                    return error_response!(
                        GetConfigResponse,
                        ResponseCode::NOT_FOUND_ERROR,
                        "Config with ID {:?} does not exist",
                        config_id
                    )
                }
            };

        let config_bytes = config.data().clone().write_to_bytes().unwrap();
        return Ok(GetConfigResponse {
            config: config_bytes,
            config_version: config.version().clone(),
            ..Default::default()
        });
    }
    /// RPC method declared in the interface.
    fn register_config_metadata(
        &self,
        request: RegisterConfigMetadataRequest,
    ) -> RpcResult<RegisterConfigMetadataResponse> {
        let mut mlock = acquire_lock_or_return_err!(self.data, RegisterConfigMetadataResponse);

        let config_id = match request.config_id.is_some() {
            true => HashableConfigId::from(request.config_id.unwrap()),
            false => {
                return error_response!(
                    RegisterConfigMetadataResponse,
                    ResponseCode::INVALID_REQUEST,
                    "ConfigId is empty",
                )
            }
        };
        if config_id.contains_empty_fields() {
            return error_response!(
                RegisterConfigMetadataResponse,
                ResponseCode::INVALID_REQUEST,
                "ConfigId fields contains an empty string: {:?}",
                config_id,
            );
        }
        if mlock.base_configs.contains_key(&config_id)
            && mlock.base_configs.get(&config_id).unwrap().version() == &request.config_version
        {
            return Ok(RegisterConfigMetadataResponse::new());
        }

        let metadata: ConfigMetadata = match request.metadata.is_some() {
            true => request.metadata.unwrap(),
            false => {
                return error_response!(
                    RegisterConfigMetadataResponse,
                    ResponseCode::INVALID_REQUEST,
                    "Metadata is not initialized",
                )
            }
        };

        if metadata.default_config.is_none() {
            return error_response!(
                RegisterConfigMetadataResponse,
                ResponseCode::INVALID_REQUEST,
                "Default configuration is not provided",
            );
        }

        if metadata.descriptor_set.is_none() {
            return error_response!(
                RegisterConfigMetadataResponse,
                ResponseCode::INVALID_REQUEST,
                "Descriptor set is not provided",
            );
        }

        let config = Configuration::new(
            metadata.default_config.unwrap().clone(),
            metadata.descriptor_set.unwrap().clone(),
            request.config_version,
        );

        mlock.base_configs.insert(config_id, config);

        Ok(RegisterConfigMetadataResponse::new())
    }
    /// RPC method declared in the interface.
    fn get_config_metadata(
        &self,
        request: GetConfigMetadataRequest,
    ) -> RpcResult<GetConfigMetadataResponse> {
        let mlock = acquire_lock_or_return_err!(self.data, GetConfigMetadataResponse);

        let config_id = match request.config_id.is_some() {
            true => HashableConfigId::from(request.config_id.unwrap()),
            false => {
                return error_response!(
                    GetConfigMetadataResponse,
                    ResponseCode::INVALID_REQUEST,
                    "ConfigId is empty",
                );
            }
        };
        if config_id.contains_empty_fields() {
            return error_response!(
                GetConfigMetadataResponse,
                ResponseCode::INVALID_REQUEST,
                "ConfigId fields contains an empty string",
            );
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
    /// RPC method declared in the interface.
    fn start_config_update(
        &self,
        _request: StartConfigUpdateRequest,
    ) -> RpcResult<StartConfigUpdateResponse> {
        let mut mlock = acquire_lock_or_return_err!(self.data, StartConfigUpdateResponse);

        mlock.current_update_id = match mlock.current_update_id {
            Some(id) => {
                return error_response!(
                    StartConfigUpdateResponse,
                    ResponseCode::SESSION_IN_PROGRESS,
                    "Update with ID {:?} already in progress",
                    id,
                );
            }
            None => Some(rand::thread_rng().gen::<i64>()),
        };

        mlock.overrides.snapshot();

        Ok(StartConfigUpdateResponse {
            update_id: mlock.current_update_id.unwrap(),
            ..Default::default()
        })
    }
    /// RPC method declared in the interface.
    fn add_or_update_config_override(
        &self,
        request: AddOrUpdateConfigOverrideRequest,
    ) -> RpcResult<AddOrUpdateConfigOverrideResponse> {
        let mut mlock = acquire_lock_or_return_err!(self.data, AddOrUpdateConfigOverrideResponse);

        if request.config_id.is_none() {
            return error_response!(
                AddOrUpdateConfigOverrideResponse,
                ResponseCode::INVALID_REQUEST,
                "ConfigId is empty",
            );
        }

        if mlock.current_update_id.is_none()
            || request.update_id != mlock.current_update_id.unwrap()
        {
            return error_response!(
                AddOrUpdateConfigOverrideResponse,
                ResponseCode::INVALID_REQUEST,
                "Invalid update request, current update has id {:?}, request has id {}",
                mlock.current_update_id,
                request.update_id
            );
        }

        for config_override in request.configOverride.iter() {
            mlock.overrides.insert(
                OverrideKey {
                    config_id: HashableConfigId::from(request.config_id.clone().unwrap()),
                    override_id: config_override.override_id.clone(),
                },
                config_override.clone(),
            );
        }

        Ok(AddOrUpdateConfigOverrideResponse::new())
    }
    /// RPC method declared in the interface.
    fn remove_config_override(
        &self,
        _request: RemoveConfigOverrideRequest,
    ) -> RpcResult<RemoveConfigOverrideResponse> {
        todo!();
    }
    /// RPC method declared in the interface.
    fn cancel_config_update(
        &self,
        request: CancelConfigUpdateRequest,
    ) -> RpcResult<CancelConfigUpdateResponse> {
        let mut mlock = acquire_lock_or_return_err!(self.data, CancelConfigUpdateResponse);

        if mlock.current_update_id.is_none()
            || request.update_id != mlock.current_update_id.unwrap()
        {
            return error_response!(
                CancelConfigUpdateResponse,
                ResponseCode::INVALID_REQUEST,
                "Invalid update request, current update has id {:?}, request has id {}",
                mlock.current_update_id,
                request.update_id
            );
        }

        mlock.current_update_id = None;
        mlock.overrides.rollback();

        Ok(CancelConfigUpdateResponse::new())
    }

    /// RPC method declared in the interface.
    fn finish_config_update(
        &self,
        request: FinishConfigUpdateRequest,
    ) -> RpcResult<FinishConfigUpdateResponse> {
        let mut mlock = acquire_lock_or_return_err!(self.data, FinishConfigUpdateResponse);

        if mlock.current_update_id.is_none()
            || request.update_id != mlock.current_update_id.unwrap()
        {
            return error_response!(
                FinishConfigUpdateResponse,
                ResponseCode::INVALID_REQUEST,
                "Current update has ID {:?} and requested update has id {:?}",
                mlock.current_update_id,
                request.update_id
            );
        }

        mlock.current_update_id = None;

        let overrides_for_each_config: HashMap<HashableConfigId, Vec<ConfigOverride>> =
            self.group_overrides_by_config_id(&mlock);
        let new_configurations =
            match self.calculate_snapshot(&mlock.base_configs, &overrides_for_each_config) {
                Ok(x) => x,
                Err(err) => {
                    return error_response!(
                        FinishConfigUpdateResponse,
                        ResponseCode::INTERNAL_ERROR,
                        "Update {:?} failed due to {:?}",
                        mlock.current_update_id,
                        err
                    )
                }
            };

        // save existing configuration
        mlock.derived_configs.snapshot();
        *mlock.derived_configs = new_configurations;

        Ok(FinishConfigUpdateResponse::new())
    }
    /// RPC method declared in the interface.
    fn rollback(&self, _request: RollbackRequest) -> RpcResult<RollbackResponse> {
        let mut mlock = acquire_lock_or_return_err!(self.data, RollbackResponse);

        if mlock.current_update_id.is_some() {
            return error_response!(
                RollbackResponse,
                ResponseCode::INVALID_REQUEST,
                "Cannot rollback while update {:?} is in progress",
                mlock.current_update_id
            );
        }

        mlock.overrides.rollback();
        mlock.derived_configs.rollback();

        Ok(RollbackResponse::new())
    }
    /// RPC method declared in the interface.
    fn reset(&self, _request: ResetRequest) -> RpcResult<ResetResponse> {
        todo!();
    }
    /// RPC method declared in the interface.
    fn deinstall_config(
        &self,
        _request: DeinstallConfigRequest,
    ) -> RpcResult<DeinstallConfigResponse> {
        todo!();
    }
    /// RPC method declared in the interface.
    fn get_calibration_config_ids(
        &self,
        _request: GetCalibrationConfigIdsRequest,
    ) -> RpcResult<GetCalibrationConfigIdsResponse> {
        todo!();
    }
    /// RPC method declared in the interface.
    fn start_calibration(
        &self,
        _request: StartCalibrationRequest,
    ) -> RpcResult<StartCalibrationResponse> {
        todo!();
    }
    /// RPC method declared in the interface.
    fn stop_calibration(
        &self,
        _request: StopCalibrationRequest,
    ) -> RpcResult<StopCalibrationResponse> {
        todo!();
    }
    /// RPC method declared in the interface.
    fn get_config_and_register_for_config_update(
        &self,
        _request: GetConfigAndRegisterForConfigUpdateRequest,
    ) -> RpcResult<GetConfigAndRegisterForConfigUpdateResponse> {
        todo!();
    }
    /// RPC method declared in the interface.
    fn unregister_from_config_update(
        &self,
        _request: UnregisterFromConfigUpdateRequest,
    ) -> RpcResult<UnregisterFromConfigUpdateResponse> {
        todo!();
    }
    /// RPC method declared in the interface.
    fn register_for_calibration_update(
        &self,
        _request: RegisterForCalibrationUpdateRequest,
    ) -> RpcResult<RegisterForCalibrationUpdateResponse> {
        todo!();
    }
    /// RPC method declared in the interface.
    fn unregister_from_calibration_update(
        &self,
        _request: UnregisterFromCalibrationUpdateRequest,
    ) -> RpcResult<UnregisterFromCalibrationUpdateResponse> {
        todo!();
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused_imports)]
    use super::*;
    use concal_api::configuration::config_override_key_value::Value;
    use concal_api::configuration::ConfigId;
    use concal_api::configuration::ConfigOverrideKeyValue;
    use protobuf::descriptor::FileDescriptorSet;
    use protobuf::well_known_types::any::Any;
    use sdvtestconcal_proto_rust::test_configuration::TestConfiguration;

    use std::path::PathBuf;

    use crate::tests::utilities::*;

    #[test]
    fn get_config_returns_error_when_request_contains_none_config_id() {
        let service = ConCalServiceImpl::new();

        let result = service
            .get_config(GetConfigRequest { ..Default::default() })
            .expect("Expected get_config to succeed");

        assert!(result.error.is_some());
        assert_eq!(
            result.error.unwrap().error_code,
            EnumOrUnknown::from(ResponseCode::INVALID_REQUEST)
        );
    }

    #[test]
    fn get_config_returns_error_when_config_doesnt_exist() {
        let config_id = test_config_id();

        let service = ConCalServiceImpl::new();

        let result = service
            .get_config(GetConfigRequest {
                config_id: MessageField::some(config_id),
                ..Default::default()
            })
            .expect("Expected get_config to succeed");

        assert!(result.error.is_some());
        assert_eq!(
            result.error.unwrap().error_code,
            EnumOrUnknown::from(ResponseCode::NOT_FOUND_ERROR)
        );
    }

    #[test]
    fn get_config_can_return_default_config() {
        let config = test_config();
        let config_id = test_config_id();

        let service = ConCalServiceImpl::new();

        let resp = register_config_metadata(&service, &config_id, &config);
        assert!(resp.error.is_none());

        let resp = get_config(&service, &config_id);
        assert!(resp.error.is_none());

        assert_eq!(&resp.config_version, config.version());
        assert_eq!(resp.config, config.data().write_to_bytes().unwrap());
    }

    #[test]
    fn calculates_derived_config_correctly() {
        let service = ConCalServiceImpl::new();
        let config = test_config();
        let config_id = test_config_id();

        let resp = register_config_metadata(&service, &config_id, &config);
        assert!(resp.error.is_none());

        let overrides = create_sample_overrides(String::from("1"), true, String::from("new_value"));

        let resp = start_config_update(&service);
        assert!(resp.error.is_none());

        let update_id = resp.update_id;

        let resp = add_or_update_config_override(&service, update_id, &config_id, &overrides);
        assert!(resp.error.is_none());

        let resp = finish_config_update(&service, update_id);
        assert!(resp.error.is_none());

        let resp = get_config(&service, &config_id);
        assert!(resp.error.is_none());
        assert_eq!(&resp.config_version, config.version());

        let any = Any::parse_from_bytes(&resp.config).expect("to convert bytes to any");
        let derived_config = TestConfiguration::parse_from_bytes(&any.value)
            .expect("to unmarshal TestConfiguration from bytes");

        assert!(derived_config.bool_parameter);
        assert_eq!(derived_config.str_parameter, String::from("new_value"));
    }

    #[test]
    fn can_rollback_to_previous_config() {
        let service = ConCalServiceImpl::new();
        let config = test_config();
        let config_id = test_config_id();

        let resp = register_config_metadata(&service, &config_id, &config);
        assert!(resp.error.is_none());

        let overrides = create_sample_overrides(String::from("1"), true, String::from("new_value"));

        let resp = start_config_update(&service);
        assert!(resp.error.is_none());

        let update_id = resp.update_id;

        let resp = add_or_update_config_override(&service, update_id, &config_id, &overrides);
        assert!(resp.error.is_none());

        let resp = finish_config_update(&service, update_id);
        assert!(resp.error.is_none());

        let resp = service.rollback(RollbackRequest::new()).expect("rollback to succeed");
        assert!(resp.error.is_none());

        let resp = get_config(&service, &config_id);
        assert!(resp.error.is_none());

        assert_eq!(&resp.config_version, config.version());
        assert_eq!(resp.config, config.data().write_to_bytes().unwrap());
    }

    fn start_config_update(service: &ConCalServiceImpl) -> StartConfigUpdateResponse {
        service
            .start_config_update(StartConfigUpdateRequest { ..Default::default() })
            .expect("start_config_update to to succeed")
    }

    fn add_or_update_config_override(
        service: &ConCalServiceImpl,
        update_id: i64,
        config_id: &ConfigId,
        overrides: &[ConfigOverride],
    ) -> AddOrUpdateConfigOverrideResponse {
        service
            .add_or_update_config_override(AddOrUpdateConfigOverrideRequest {
                update_id,
                config_id: MessageField::some(config_id.clone()),
                configOverride: overrides.to_vec(),
                ..Default::default()
            })
            .expect("add_or_update_config_override to succeed")
    }

    fn finish_config_update(
        service: &ConCalServiceImpl,
        update_id: i64,
    ) -> FinishConfigUpdateResponse {
        service
            .finish_config_update(FinishConfigUpdateRequest { update_id, ..Default::default() })
            .expect("finish_config_update to succeed")
    }

    fn get_config(service: &ConCalServiceImpl, config_id: &ConfigId) -> GetConfigResponse {
        service
            .get_config(GetConfigRequest {
                config_id: MessageField::some(config_id.clone()),
                ..Default::default()
            })
            .expect("get_config to succeed")
    }

    fn create_sample_overrides(
        override_id: String,
        bool_parameter: bool,
        str_parameter: String,
    ) -> Vec<ConfigOverride> {
        vec![ConfigOverride {
            override_id,
            pairs: vec![
                ConfigOverrideKeyValue {
                    key: String::from("bool_parameter"),
                    value: Some(Value::ValueBool(bool_parameter)),
                    ..Default::default()
                },
                ConfigOverrideKeyValue {
                    key: String::from("str_parameter"),
                    value: Some(Value::ValueStr(str_parameter)),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }]
    }

    fn register_config_metadata(
        service: &ConCalServiceImpl,
        id: &ConfigId,
        config: &Configuration,
    ) -> RegisterConfigMetadataResponse {
        service
            .register_config_metadata(RegisterConfigMetadataRequest {
                config_id: MessageField::some(id.clone()),
                metadata: MessageField::some(ConfigMetadata {
                    descriptor_set: MessageField::some(config.descriptor().clone()),
                    default_config: MessageField::some(config.data().clone()),
                    ..Default::default()
                }),
                config_version: config.version().clone(),
                ..Default::default()
            })
            .expect("call to register_config_metadata to succeed")
    }
}
