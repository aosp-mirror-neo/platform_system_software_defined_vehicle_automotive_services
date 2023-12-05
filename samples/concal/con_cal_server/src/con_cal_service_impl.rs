use concal_api::calibration::*;
use concal_api::concal_service::*;
use concal_api::configuration::ConfigMetadata;
use concal_api::configuration::ConfigOverride;
use concal_api::error::{Error, ResponseCode};
use protobuf::EnumOrUnknown;
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

macro_rules! acquire_lock_or_return_err {
    ($mutex:expr,$return_type:ident) => {{
        match $mutex.lock() {
            Ok(x) => x,
            Err(e) => {
                return Ok($return_type {
                    error: ConCalServiceImpl::error(
                        ResponseCode::RUNTIME_ERROR,
                        String::from("Unable to obtain mutex"),
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

#[derive(Debug, Hash, Eq, PartialEq)]
struct OverrideKey {
    config_id: HashableConfigId,
    override_id: String,
}

#[derive(Default)]
struct ConCalServiceData {
    base_configs: HashMap<HashableConfigId, Configuration>,
    derived_configs: HashMap<HashableConfigId, Configuration>,
    overrides: HashMap<OverrideKey, ConfigOverride>,
    current_update_id: Option<i64>,
}

#[derive(Default)]
pub struct ConCalServiceImpl {
    data: Mutex<ConCalServiceData>,
}

impl ConCalServiceImpl {
    pub fn new() -> Self {
        ConCalServiceImpl { ..Default::default() }
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

    fn compute_new_configs(
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
        todo!();
    }
    /// RPC method declared in the interface.
    fn register_config_metadata(
        &self,
        request: RegisterConfigMetadataRequest,
    ) -> RpcResult<RegisterConfigMetadataResponse> {
        let mut mlock = match self.data.lock() {
            Ok(x) => x,
            Err(e) => {
                return Ok(RegisterConfigMetadataResponse {
                    error: ConCalServiceImpl::error(
                        ResponseCode::INTERNAL_ERROR,
                        String::from("Unable to obtain mutex"),
                    ),
                    ..Default::default()
                })
            }
        };

        let config_id = match request.config_id.is_some() {
            true => HashableConfigId::from(request.config_id.unwrap()),
            false => {
                return Ok(RegisterConfigMetadataResponse {
                    error: ConCalServiceImpl::error(
                        ResponseCode::INVALID_REQUEST,
                        String::from("ConfigId is empty"),
                    ),
                    ..Default::default()
                })
            }
        };
        if config_id.contains_empty_fields() {
            return Ok(RegisterConfigMetadataResponse {
                error: ConCalServiceImpl::error(
                    ResponseCode::INVALID_REQUEST,
                    String::from("ConfigId fields contains an empty string"),
                ),
                ..Default::default()
            });
        }
        if mlock.base_configs.contains_key(&config_id)
            && mlock.base_configs.get(&config_id).unwrap().version() == &request.config_version
        {
            return Ok(RegisterConfigMetadataResponse::new());
        }

        let metadata: ConfigMetadata = match request.metadata.is_some() {
            true => request.metadata.unwrap(),
            false => {
                return Ok(RegisterConfigMetadataResponse {
                    error: ConCalServiceImpl::error(
                        ResponseCode::INVALID_REQUEST,
                        String::from("Metadata is not initialized"),
                    ),
                    ..Default::default()
                })
            }
        };
        if metadata.default_config.is_none() {
            return Ok(RegisterConfigMetadataResponse {
                error: ConCalServiceImpl::error(
                    ResponseCode::INVALID_REQUEST,
                    String::from("Default configuration is not provided"),
                ),
                ..Default::default()
            });
        }

        if metadata.descriptor_set.is_none() {
            return Ok(RegisterConfigMetadataResponse {
                error: ConCalServiceImpl::error(
                    ResponseCode::INVALID_REQUEST,
                    String::from("Descriptor set is not provided"),
                ),
                ..Default::default()
            });
        }

        let config = Configuration::new(
            metadata.default_config.unwrap().clone(),
            metadata.descriptor_set.unwrap().clone(),
            request.config_version,
        );

        mlock.base_configs.insert(config_id, config);

        return Ok(RegisterConfigMetadataResponse::new());
    }
    /// RPC method declared in the interface.
    fn get_config_metadata(
        &self,
        request: GetConfigMetadataRequest,
    ) -> RpcResult<GetConfigMetadataResponse> {
        let mut mlock = match self.data.lock() {
            Ok(x) => x,
            //TODO: return error here
            Err(e) => return Ok(GetConfigMetadataResponse::new()),
        };
        let config_id = match request.config_id.is_some() {
            true => HashableConfigId::from(request.config_id.unwrap()),
            false => {
                return Ok(GetConfigMetadataResponse {
                    error: ConCalServiceImpl::error(
                        ResponseCode::INVALID_REQUEST,
                        String::from("ConfigId is empty"),
                    ),
                    ..Default::default()
                })
            }
        };
        if config_id.contains_empty_fields() {
            return Ok(GetConfigMetadataResponse {
                error: ConCalServiceImpl::error(
                    ResponseCode::INVALID_REQUEST,
                    String::from("ConfigId fields contains an empty string"),
                ),
                ..Default::default()
            });
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

        return Ok(response);
    }
    /// RPC method declared in the interface.
    fn start_config_update(
        &self,
        request: StartConfigUpdateRequest,
    ) -> RpcResult<StartConfigUpdateResponse> {
        let mut mlock = match self.data.lock() {
            Ok(x) => x,
            Err(x) => {
                return Ok(StartConfigUpdateResponse {
                    error: ConCalServiceImpl::error(
                        ResponseCode::INTERNAL_ERROR,
                        String::from("Unable to obtain mutex"),
                    ),
                    ..Default::default()
                });
            }
        };

        mlock.current_update_id = match mlock.current_update_id {
            Some(x) => {
                return Ok(StartConfigUpdateResponse {
                    error: ConCalServiceImpl::error(
                        ResponseCode::SESSION_IN_PROGRESS,
                        format!("Update with ID {:?} already in progress", x),
                    ),
                    ..Default::default()
                });
            }
            None => Some(rand::thread_rng().gen::<i64>()),
        };

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
        let mut mlock = match self.data.lock() {
            Ok(x) => x,
            Err(e) => {
                return Ok(AddOrUpdateConfigOverrideResponse {
                    error: ConCalServiceImpl::error(
                        ResponseCode::INTERNAL_ERROR,
                        String::from("Unable to obtain mutex"),
                    ),
                    ..Default::default()
                })
            }
        };

        if request.config_id.is_none() {
            return Ok(AddOrUpdateConfigOverrideResponse {
                error: ConCalServiceImpl::error(
                    ResponseCode::INVALID_REQUEST,
                    String::from("ConfigId is empty"),
                ),
                ..Default::default()
            });
        }

        if mlock.current_update_id.is_none()
            || request.update_id != mlock.current_update_id.unwrap()
        {
            return Ok(AddOrUpdateConfigOverrideResponse {
                error: ConCalServiceImpl::error(
                    ResponseCode::INVALID_REQUEST,
                    format!(
                        "Invalid update request, current update has id {:?}, request has id {}",
                        mlock.current_update_id, request.update_id
                    ),
                ),
                ..Default::default()
            });
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

        return Ok(AddOrUpdateConfigOverrideResponse::new());
    }
    /// RPC method declared in the interface.
    fn remove_config_override(
        &self,
        request: RemoveConfigOverrideRequest,
    ) -> RpcResult<RemoveConfigOverrideResponse> {
        todo!();
    }
    /// RPC method declared in the interface.
    fn cancel_config_update(
        &self,
        request: CancelConfigUpdateRequest,
    ) -> RpcResult<CancelConfigUpdateResponse> {
        todo!();
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
            self.compute_new_configs(&mlock.base_configs, &overrides_for_each_config);

        if let Err(err) = new_configurations {
            return error_response!(
                FinishConfigUpdateResponse,
                ResponseCode::INTERNAL_ERROR,
                "Update {:?} failed due to {:?}",
                mlock.current_update_id,
                err
            );
        }

        mlock.derived_configs = new_configurations.unwrap();

        Ok(FinishConfigUpdateResponse::new())
    }
    /// RPC method declared in the interface.
    fn rollback(&self, request: RollbackRequest) -> RpcResult<RollbackResponse> {
        todo!();
    }
    /// RPC method declared in the interface.
    fn reset(&self, request: ResetRequest) -> RpcResult<ResetResponse> {
        todo!();
    }
    /// RPC method declared in the interface.
    fn deinstall_config(
        &self,
        request: DeinstallConfigRequest,
    ) -> RpcResult<DeinstallConfigResponse> {
        todo!();
    }
    /// RPC method declared in the interface.
    fn get_calibration_config_ids(
        &self,
        request: GetCalibrationConfigIdsRequest,
    ) -> RpcResult<GetCalibrationConfigIdsResponse> {
        todo!();
    }
    /// RPC method declared in the interface.
    fn start_calibration(
        &self,
        request: StartCalibrationRequest,
    ) -> RpcResult<StartCalibrationResponse> {
        todo!();
    }
    /// RPC method declared in the interface.
    fn stop_calibration(
        &self,
        request: StopCalibrationRequest,
    ) -> RpcResult<StopCalibrationResponse> {
        todo!();
    }
    /// RPC method declared in the interface.
    fn get_config_and_register_for_config_update(
        &self,
        request: GetConfigAndRegisterForConfigUpdateRequest,
    ) -> RpcResult<GetConfigAndRegisterForConfigUpdateResponse> {
        todo!();
    }
    /// RPC method declared in the interface.
    fn unregister_from_config_update(
        &self,
        request: UnregisterFromConfigUpdateRequest,
    ) -> RpcResult<UnregisterFromConfigUpdateResponse> {
        todo!();
    }
    /// RPC method declared in the interface.
    fn register_for_calibration_update(
        &self,
        request: RegisterForCalibrationUpdateRequest,
    ) -> RpcResult<RegisterForCalibrationUpdateResponse> {
        todo!();
    }
    /// RPC method declared in the interface.
    fn unregister_from_calibration_update(
        &self,
        request: UnregisterFromCalibrationUpdateRequest,
    ) -> RpcResult<UnregisterFromCalibrationUpdateResponse> {
        todo!();
    }
}
