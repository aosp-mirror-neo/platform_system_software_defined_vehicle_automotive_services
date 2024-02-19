// Copyright 2024 Google LLC
use crate::diagnostics_translator::DiagnosticsTranslatorError::{
    MissingEntityDeclarationError, MissingServiceBundleDeclarationError, VsidlError,
};
use crate::entity_id::EntityId;
use crate::entity_id::EntityId::DataItem;
use crate::translator::{Translator, TranslatorError, TranslatorImpl};
use protobuf::reflect::MessageDescriptor;
use sdv_config_provider::{VsidlConfigProvider, VsidlProvider};
use vsidl_syntax::diagnostics::DiagnosticsDeclaration;

/// UDS to SDV translator for diagnostic entities.
/// Provides methods to translate protobuf bytes to UDS-compatible bytes to and back,
/// taking diagnostics entity id as a description of message format.
pub struct DiagnosticsTranslator {
    vsidl_provider: Box<dyn VsidlConfigProvider>,
    translator: Box<dyn Translator>,
}

impl DiagnosticsTranslator {
    /// Creates new instance.
    pub fn new() -> Self {
        Self::init(Box::new(VsidlProvider::new().unwrap()), Box::new(TranslatorImpl))
    }

    fn init(vsidl_provider: Box<dyn VsidlConfigProvider>, translator: Box<dyn Translator>) -> Self {
        Self { vsidl_provider, translator }
    }

    /// Translates serialized protobuf message `proto_bytes` to UDS message.
    /// Method uses `entity_id` to perform reflection-based translation.
    /// Returns serialized UDS message or `TranslatorError`.
    ///
    /// `proto_bytes` are expected to be in format produced by
    /// `MessageDyn::write_to_bytes()`.
    pub fn translate_from_protobuf_to_uds(
        &self,
        service_fqbn: &str,
        entity_id: EntityId,
        proto_bytes: &[u8],
    ) -> Result<Vec<u8>, DiagnosticsTranslatorError> {
        let message_descriptor = self.find_descriptor(service_fqbn, entity_id)?;

        Ok(self.translator.translate_from_protobuf_to_uds(proto_bytes, &message_descriptor)?)
    }

    /// Translates serialized UDS message `proto_bytes` to protobuf message.
    /// Method uses `entity_id` to perform reflection-based translation.
    /// Returns serialized protobuf message or `TranslatorError`.
    ///
    /// Returned proto bytes are in format produced by `MessageDyn::write_to_bytes()`.
    pub fn translate_from_uds_to_protobuf(
        &self,
        service_fqbn: &str,
        entity_id: EntityId,
        uds_bytes: &[u8],
    ) -> Result<Vec<u8>, DiagnosticsTranslatorError> {
        let message_descriptor = self.find_descriptor(service_fqbn, entity_id)?;

        Ok(self.translator.translate_from_uds_to_protobuf(uds_bytes, &message_descriptor)?)
    }

    fn find_descriptor(
        &self,
        service_fqbn: &str,
        entity_id: EntityId,
    ) -> Result<MessageDescriptor, DiagnosticsTranslatorError> {
        let diagnostics_declarations = self.vsidl_provider.get_diagnostics_declarations();

        let diagnostics_declaration = diagnostics_declarations
            .get(service_fqbn)
            .ok_or_else(|| MissingServiceBundleDeclarationError(service_fqbn.to_string()))?;

        let message_name = Self::extract_message_name(diagnostics_declaration, &entity_id)
            .ok_or_else(|| MissingEntityDeclarationError(service_fqbn.to_string(), entity_id))?;

        self.vsidl_provider
            .get_message_descriptor(message_name)
            .map_err(|vsidl_provider_error| VsidlError(vsidl_provider_error.to_string()))
    }

    fn extract_message_name<'lt>(
        declaration: &'lt DiagnosticsDeclaration,
        entity_id: &EntityId,
    ) -> Option<&'lt str> {
        match entity_id {
            DataItem(id) => declaration
                .data_item
                .iter()
                .find(|data_item| &data_item.id == id)
                .map(|data_item| data_item.message_name.as_str()),
        }
    }
}

impl Default for DiagnosticsTranslator {
    fn default() -> Self {
        Self::new()
    }
}

/// Error type for translation between protobuf and UDS formats
#[derive(Debug, thiserror::Error)]
pub enum DiagnosticsTranslatorError {
    /// Error getting information from VSIDL
    #[error("Error getting information from VSIDL: {0}")]
    VsidlError(String),

    /// Declaration of service bundle is missing
    #[error("Diagnostics declaration of [{0}] is missing")]
    MissingServiceBundleDeclarationError(String),

    /// Declaration corresponding to diagnostics message is missing from declaration
    #[error("Diagnostics entity declaration [{1:#?}] is missing from declaration of [{0}]")]
    MissingEntityDeclarationError(String, EntityId),

    /// Error when translating message
    #[error("Translation error: {0:#?}")]
    TranslationError(#[from] TranslatorError),
}

#[cfg(test)]
mod tests {
    use crate::diagnostics_translator::{DiagnosticsTranslator, DiagnosticsTranslatorError::*};
    use crate::entity_id::EntityId;
    use crate::translator::runtime_message_descriptor::get_file_descriptor;
    use crate::translator::MockTranslator;
    use googletest::assert_that;
    use googletest::prelude::{eq, matches_pattern};
    use mockall::predicate::{always, eq as mock_eq};
    use sdv_config_provider::{MockVsidlConfigProvider, VsidlProviderError};
    use std::collections::HashMap;
    use vsidl_syntax::diagnostics::{diagnostics_declaration::DataItem, DiagnosticsDeclaration};

    const MESSAGE_BYTES: [u8; 5] = [1, 2, 3, 4, 5];
    const DATA_ITEM_MESSAGE: &str = "DataItem";

    fn create_diagnostics_translator(
        service_fqbn: String,
        declaration: DiagnosticsDeclaration,
    ) -> DiagnosticsTranslator {
        let file_descriptor = get_file_descriptor(
            r#"syntax = 'proto3';

            message DataItem {}
            "#,
        );

        let mut mock_vsidl_provider = MockVsidlConfigProvider::new();
        mock_vsidl_provider.expect_get_diagnostics_declarations().returning(move || {
            let mut result = HashMap::new();
            result.insert(service_fqbn.to_string(), declaration.clone());
            result
        });
        mock_vsidl_provider.expect_get_message_descriptor().returning(move |message_name| {
            file_descriptor
                .message_by_full_name(message_name)
                .ok_or(VsidlProviderError::Error("Message not found!".to_string()))
        });

        let mut mock_translator = MockTranslator::new();
        mock_translator
            .expect_translate_from_protobuf_to_uds()
            .with(mock_eq(MESSAGE_BYTES), always())
            .returning(|_, descriptor| Ok(uds_bytes_for_message(descriptor.name())));
        mock_translator
            .expect_translate_from_uds_to_protobuf()
            .with(mock_eq(MESSAGE_BYTES), always())
            .returning(|_, descriptor| Ok(proto_bytes_for_message(descriptor.name())));

        DiagnosticsTranslator::init(Box::new(mock_vsidl_provider), Box::new(mock_translator))
    }

    fn proto_bytes_for_message(message_name: &str) -> Vec<u8> {
        ("proto:".to_owned() + message_name).to_string().as_bytes().to_vec()
    }

    fn uds_bytes_for_message(message_name: &str) -> Vec<u8> {
        ("uds:".to_owned() + message_name).to_string().as_bytes().to_vec()
    }

    #[googletest::test]
    fn translate_data_item_to_uds() {
        let translator = create_diagnostics_translator(
            "service_fqbn".to_string(),
            DiagnosticsDeclaration {
                data_item: vec![DataItem {
                    id: "data_item_id".to_string(),
                    message_name: format!(".{}", DATA_ITEM_MESSAGE),
                    ..Default::default()
                }],
                ..Default::default()
            },
        );

        assert_that!(
            translator.translate_from_protobuf_to_uds(
                "service_fqbn",
                EntityId::DataItem("data_item_id".to_string()),
                &MESSAGE_BYTES[..]
            ),
            matches_pattern!(Ok(eq(uds_bytes_for_message(DATA_ITEM_MESSAGE))))
        );
    }

    #[googletest::test]
    fn translate_data_item_to_protobuf() {
        let translator = create_diagnostics_translator(
            "service_fqbn".to_string(),
            DiagnosticsDeclaration {
                data_item: vec![DataItem {
                    id: "data_item_id".to_string(),
                    message_name: format!(".{}", DATA_ITEM_MESSAGE),
                    ..Default::default()
                }],
                ..Default::default()
            },
        );

        assert_that!(
            translator.translate_from_uds_to_protobuf(
                "service_fqbn",
                EntityId::DataItem("data_item_id".to_string()),
                &MESSAGE_BYTES[..]
            ),
            matches_pattern!(Ok(eq(proto_bytes_for_message(DATA_ITEM_MESSAGE))))
        );
    }

    #[googletest::test]
    fn translate_service_declaration_missing() {
        let translator = create_diagnostics_translator(
            "service_fqbn1".to_string(),
            DiagnosticsDeclaration { ..Default::default() },
        );

        assert_that!(
            translator.translate_from_uds_to_protobuf(
                "service_fqbn",
                EntityId::DataItem("data_item_id".to_string()),
                &MESSAGE_BYTES[..]
            ),
            matches_pattern!(Err(matches_pattern!(MissingServiceBundleDeclarationError(eq(
                "service_fqbn".to_string()
            )))))
        );
    }

    #[googletest::test]
    fn translate_entity_declaration_missing() {
        let translator = create_diagnostics_translator(
            "service_fqbn".to_string(),
            DiagnosticsDeclaration { ..Default::default() },
        );

        assert_that!(
            translator.translate_from_uds_to_protobuf(
                "service_fqbn",
                EntityId::DataItem("data_item_id".to_string()),
                &MESSAGE_BYTES[..]
            ),
            matches_pattern!(Err(matches_pattern!(MissingEntityDeclarationError(
                eq("service_fqbn".to_string()),
                matches_pattern!(EntityId::DataItem(eq("data_item_id".to_string())))
            ))))
        );
    }
}
