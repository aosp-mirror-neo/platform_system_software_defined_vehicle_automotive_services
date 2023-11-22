// Copyright 2023 Google LLC
#![allow(unused_variables)]

use crate::not_implemented;
use android_sdv_diagnostics_platform::aidl::android::sdv::diagnostics::{
    authentication::IAuthenticationHandler::IAuthenticationHandler,
    ecu_reset::IECUResetHandler::IECUResetHandler,
    file_transfer::IFileTransferHandler::IFileTransferHandler,
    metadata::ConnectionMetadata::ConnectionMetadata,
    security_access::ISecurityAccessHandler::ISecurityAccessHandler,
    IDiagnosticsManagerPlatform::IDiagnosticsManagerPlatformAsyncServer,
    ResponseCode::ResponseCode,
};
use android_sdv_diagnostics_platform::binder::{self, Interface, Result, Strong};

pub struct DiagnosticsManagerPlatformAsync;
impl Interface for DiagnosticsManagerPlatformAsync {}
#[::async_trait::async_trait]
impl IDiagnosticsManagerPlatformAsyncServer for DiagnosticsManagerPlatformAsync {
    async fn registerECUResetHandler(
        &self,
        id: &str,
        handler: &Strong<dyn IECUResetHandler>,
    ) -> Result<()> {
        not_implemented!()
    }

    async fn registerSecurityAccessHandler(
        &self,
        id: &str,
        handler: &Strong<dyn ISecurityAccessHandler>,
    ) -> Result<()> {
        not_implemented!()
    }

    async fn registerAuthenticationHandler(
        &self,
        id: &str,
        handler: &Strong<dyn IAuthenticationHandler>,
    ) -> Result<()> {
        not_implemented!()
    }

    async fn registerFileTransferHandler(
        &self,
        id: &str,
        handler: &Strong<dyn IFileTransferHandler>,
    ) -> Result<()> {
        not_implemented!()
    }

    async fn getConnectionMetadata(&self, connectionId: i32) -> Result<ConnectionMetadata> {
        not_implemented!()
    }
}
