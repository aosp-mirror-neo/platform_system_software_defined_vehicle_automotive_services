// Copyright 2024 Google LLC
//! Sample user preferences implementation
use com_sdv_google_user_preferences_user_preferences_admin_service_rpc::Interface as AdminService;
use googletest::prelude::*;
use protobuf::MessageField;
use sdv_comms::id::ServiceFqin;
use std::sync::Arc;
use user_preferences_api::setting::SettingKind;
use user_preferences_api::user::User as UserProto;
use user_preferences_api::user_preferences_admin_service::*;

use crate::admin_service_impl::AdminServiceImpl;
use crate::persistence::setting_definition::SettingDefinition;
use crate::persistence::settings_group_repository::SettingsGroup;
use crate::persistence::{initialize_persistence, Persistence};
use crate::rpc_client_factory::MockRpcClientFactory;
use crate::settings_change_propagator_impl::MockSettingsChangePropagator;

pub struct TestSetup {
    pub rpc_client_factory: Arc<MockRpcClientFactory>,
    pub settings_change_propagator: Arc<MockSettingsChangePropagator>,
    pub persistence: Arc<Persistence>,
}

impl TestSetup {
    pub fn new() -> Self {
        let rpc_client_factory = Arc::new(MockRpcClientFactory::new());
        let settings_change_propagator = Arc::new(MockSettingsChangePropagator::new());
        let persistence =
            Arc::new(initialize_persistence(rusqlite::Connection::open_in_memory().unwrap()));

        Self { rpc_client_factory, settings_change_propagator, persistence }
    }

    pub fn mut_rpc_client_factory(&mut self) -> &mut MockRpcClientFactory {
        Arc::get_mut(&mut self.rpc_client_factory)
            .expect("Unable to get mutable ref to rpc client factory")
    }

    pub fn mut_settings_change_propagator(&mut self) -> &mut MockSettingsChangePropagator {
        Arc::get_mut(&mut self.settings_change_propagator)
            .expect("Unable to get mutable ref to MockSettingsChangePropagator")
    }

    pub fn populate_persistence_with_settings_definitions(
        &self,
    ) -> (SettingsGroup, Vec<SettingDefinition>) {
        let group = SettingsGroup {
            name: "display".to_string(),
            service_fqin: get_user_controllable_service_fqin().to_string(),
            version: "1.0".to_string(),
            id: 0,
        };

        let definitions = vec![
            SettingDefinition {
                kind: SettingKind::PER_USER as i32,
                name: "brightness".to_string(),
                default_value: crate::persistence::setting::Value::Int32(50),
                constraint: None,
            },
            SettingDefinition {
                kind: SettingKind::PER_USER as i32,
                name: "contrast".to_string(),
                default_value: crate::persistence::setting::Value::Int32(50),
                constraint: None,
            },
        ];

        let group_id =
            self.persistence.setting_groups.store(&group).expect("Failed to store settings group");
        self.persistence
            .setting_definitions
            .store(group_id, &definitions)
            .expect("Failed to store setting definition");

        (group, definitions)
    }
}

pub async fn create_user_and_assert_success(service: &AdminServiceImpl, id: i32, flags: i32) {
    let user_proto = UserProto { id, flags, ..Default::default() };
    let create_user_request =
        CreateUserRequest { user: MessageField::some(user_proto), ..Default::default() };
    let response = service.CreateUser(get_client_fqin(), &create_user_request).await;

    assert_that!(response, ok(anything()));
}

pub async fn select_user_and_assert_success(service: &AdminServiceImpl, user_id: i32) {
    let request = SelectUserRequest { user_id, ..Default::default() };
    let response = service.SelectUser(get_client_fqin(), &request).await;
    assert_that!(response, ok(anything()));
}

pub fn create_service_impl(test_setup: &TestSetup) -> AdminServiceImpl {
    AdminServiceImpl::new(
        test_setup.persistence.clone(),
        test_setup.rpc_client_factory.clone(),
        test_setup.settings_change_propagator.clone(),
    )
}

pub fn get_user_controllable_service_fqin() -> ServiceFqin {
    ServiceFqin::builder()
        .sdv_vm_name("vm")
        .sdv_package_name("com.sdv.oem.user_preferences")
        .service_bundle_name("UserControllableService")
        .service_instance_name("default")
        .build()
        .unwrap()
}

pub fn get_client_fqin() -> ServiceFqin {
    ServiceFqin::builder()
        .sdv_vm_name("vm")
        .sdv_package_name("com.sdv.oem.user_preferences")
        .service_bundle_name("UserPreferencesClient")
        .service_instance_name("default")
        .build()
        .unwrap()
}
