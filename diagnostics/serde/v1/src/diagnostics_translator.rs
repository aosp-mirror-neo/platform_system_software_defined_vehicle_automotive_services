// Copyright 2024 Google LLC
use crate::diagnostics_translator::DiagnosticsTranslatorError::{
    MissingEntityDeclarationError, MissingServiceBundleDeclarationError, VsidlProviderError,
};
use crate::entity_id::EntityId::{self, *};
use crate::entity_id::MethodMessage::*;
use crate::entity_id::RoutineMethod::*;
use crate::translator::{Translator, TranslatorError, TranslatorImpl};
use protobuf::reflect::MessageDescriptor;
use sdv_comms::id::ServiceFqin;
use sdv_comms::ContextRef;
use sdv_config_provider::{
    vsidl_provider::VsidlProviderV1, vsidl_provider_api::VsidlConfigProviderV1,
};
use vsidl_v1::diagnostics::DiagnosticsDeclaration;

/// UDS to SDV translator for diagnostic entities.
/// Provides methods to translate protobuf bytes to UDS-compatible bytes to and back,
/// taking diagnostics entity id as a description of message format.
pub struct DiagnosticsTranslator {
    vsidl_provider: Box<dyn VsidlConfigProviderV1 + Send + Sync>,
    translator: Box<dyn Translator + Send + Sync>,
}

impl DiagnosticsTranslator {
    /// Creates new instance.
    pub fn new(context: Option<ContextRef>) -> Self {
        Self::init(
            Box::new(VsidlProviderV1::new_sync_provider(context).unwrap()),
            Box::new(TranslatorImpl),
        )
    }

    fn init(
        vsidl_provider: Box<dyn VsidlConfigProviderV1 + Send + Sync>,
        translator: Box<dyn Translator + Send + Sync>,
    ) -> Self {
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
        service_fqbn: &ServiceFqin,
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
        service_fqbn: &ServiceFqin,
        entity_id: EntityId,
        uds_bytes: &[u8],
    ) -> Result<Vec<u8>, DiagnosticsTranslatorError> {
        let message_descriptor = self.find_descriptor(service_fqbn, entity_id)?;

        Ok(self.translator.translate_from_uds_to_protobuf(uds_bytes, &message_descriptor)?)
    }

    fn find_descriptor(
        &self,
        service_fqbn: &ServiceFqin,
        entity_id: EntityId,
    ) -> Result<MessageDescriptor, DiagnosticsTranslatorError> {
        let diagnostics_declaration = self
            .vsidl_provider
            .get_diagnostics_declaration(service_fqbn)
            .ok_or_else(|| MissingServiceBundleDeclarationError(service_fqbn.to_string()))?;

        let message_name = Self::extract_message_name(&diagnostics_declaration, &entity_id)
            .ok_or_else(|| MissingEntityDeclarationError(service_fqbn.to_string(), entity_id))?;

        self.vsidl_provider
            .get_message_descriptor(service_fqbn, message_name)
            .ok_or_else(|| VsidlProviderError(service_fqbn.to_string(), message_name.to_string()))
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
            InputOutputData(id) => declaration
                .io_control
                .iter()
                .find(|io_control| &io_control.id == id)
                .map(|io_control| io_control.message_name.as_str()),
            Routine { id, method, message } => declaration
                .routine
                .iter()
                .find(|routine| &routine.id == id)
                .map(|routine| match method {
                    Start => &routine.start,
                    Stop => &routine.stop,
                    RequestResult => &routine.result,
                })
                .map(|routine_method| match message {
                    Request => &routine_method.request,
                    Response => &routine_method.response,
                })
                .map(|message_name| message_name.as_str()),
            EventExtendedData(id) => declaration
                .event
                .iter()
                .find(|event| &event.id == id)
                .map(|event| event.extended_data_message_name.as_str()),
        }
    }
}

impl Default for DiagnosticsTranslator {
    fn default() -> Self {
        Self::new(None)
    }
}

/// Error type for translation between protobuf and UDS formats
#[derive(Debug, thiserror::Error)]
pub enum DiagnosticsTranslatorError {
    /// Error getting information from VSIDL
    #[error("Error getting information from VSIDL provider for message [{1}] from source [{0}]")]
    VsidlProviderError(String, String),

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
    use crate::entity_id::*;
    use crate::translator::runtime_message_descriptor::get_file_descriptor;
    use crate::translator::MockTranslator;
    use googletest::assert_that;
    use googletest::prelude::{eq, matches_pattern};
    use mockall::predicate::{always, eq as mock_eq};
    use protobuf::MessageField;
    use sdv_comms::id::ServiceFqin;
    use sdv_config_provider::vsidl_provider_api::MockVsidlConfigProviderV1;
    use std::str::FromStr;
    use vsidl_v1::diagnostics::{
        diagnostics_declaration::{routine::Method, *},
        DiagnosticsDeclaration,
    };

    const MESSAGE_BYTES: [u8; 5] = [1, 2, 3, 4, 5];
    const DATA_ITEM_MESSAGE: &str = "DataItem";
    const IO_CONTROL_MESSAGE: &str = "IoControl";
    const ROUTINE_START_REQUEST: &str = "RoutineStartRequest";
    const ROUTINE_START_RESPONSE: &str = "RoutineStartResponse";
    const ROUTINE_STOP_REQUEST: &str = "RoutineStopRequest";
    const ROUTINE_STOP_RESPONSE: &str = "RoutineStopResponse";
    const ROUTINE_RESULT_REQUEST: &str = "RoutineResultRequest";
    const ROUTINE_RESULT_RESPONSE: &str = "RoutineResultResponse";
    const EVENT_EXTENDED_DATA: &str = "EventExtendedData";

    fn create_diagnostics_translator(
        service_fqbn: ServiceFqin,
        declaration: DiagnosticsDeclaration,
    ) -> DiagnosticsTranslator {
        let file_descriptor = get_file_descriptor(
            r#"syntax = 'proto3';

            message DataItem {}
            message IoControl {}
            message RoutineStartRequest{};
            message RoutineStartResponse{};
            message RoutineStopRequest{};
            message RoutineStopResponse{};
            message RoutineResultRequest{};
            message RoutineResultResponse{};
            message EventExtendedData{};
            "#,
        );

        let mut mock_vsidl_provider = MockVsidlConfigProviderV1::new();
        mock_vsidl_provider.expect_get_diagnostics_declaration().returning(move |fqin| {
            if *fqin == service_fqbn {
                Some(declaration.clone())
            } else {
                None
            }
        });
        mock_vsidl_provider.expect_get_message_descriptor().returning(
            move |_source_fqin, message_name| file_descriptor.message_by_full_name(message_name),
        );

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

    fn data_item_declaration(data_item_id: &str) -> DiagnosticsDeclaration {
        DiagnosticsDeclaration {
            data_item: vec![DataItem {
                id: data_item_id.to_string(),
                message_name: format!(".{}", DATA_ITEM_MESSAGE),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[googletest::test]
    fn translate_data_item_to_uds() {
        let translator = create_diagnostics_translator(
            service_fqin("ServiceBundleName"),
            data_item_declaration("data_item_id"),
        );

        assert_that!(
            translator.translate_from_protobuf_to_uds(
                &service_fqin("ServiceBundleName"),
                EntityId::DataItem("data_item_id".to_string()),
                &MESSAGE_BYTES[..]
            ),
            matches_pattern!(Ok(eq(uds_bytes_for_message(DATA_ITEM_MESSAGE))))
        );
    }

    #[googletest::test]
    fn translate_data_item_to_protobuf() {
        let translator = create_diagnostics_translator(
            service_fqin("ServiceBundleName"),
            data_item_declaration("data_item_id"),
        );

        assert_that!(
            translator.translate_from_uds_to_protobuf(
                &service_fqin("ServiceBundleName"),
                EntityId::DataItem("data_item_id".to_string()),
                &MESSAGE_BYTES[..]
            ),
            matches_pattern!(Ok(eq(proto_bytes_for_message(DATA_ITEM_MESSAGE))))
        );
    }

    fn io_control_declaration(io_control_id: &str) -> DiagnosticsDeclaration {
        DiagnosticsDeclaration {
            io_control: vec![InputOutputControlData {
                id: io_control_id.to_string(),
                message_name: format!(".{}", IO_CONTROL_MESSAGE),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[googletest::test]
    fn translate_io_control_data_to_uds() {
        let translator = create_diagnostics_translator(
            service_fqin("ServiceBundleName"),
            io_control_declaration("io_data"),
        );

        assert_that!(
            translator.translate_from_protobuf_to_uds(
                &service_fqin("ServiceBundleName"),
                EntityId::InputOutputData("io_data".to_string()),
                &MESSAGE_BYTES[..]
            ),
            matches_pattern!(Ok(eq(uds_bytes_for_message(IO_CONTROL_MESSAGE))))
        );
    }

    #[googletest::test]
    fn translate_io_control_data_to_protobuf() {
        let translator = create_diagnostics_translator(
            service_fqin("ServiceBundleName"),
            io_control_declaration("io_data"),
        );

        assert_that!(
            translator.translate_from_uds_to_protobuf(
                &service_fqin("ServiceBundleName"),
                EntityId::InputOutputData("io_data".to_string()),
                &MESSAGE_BYTES[..]
            ),
            matches_pattern!(Ok(eq(proto_bytes_for_message(IO_CONTROL_MESSAGE))))
        );
    }

    fn routine_declaration(routine_id: &str) -> DiagnosticsDeclaration {
        DiagnosticsDeclaration {
            routine: vec![Routine {
                id: routine_id.to_string(),
                start: MessageField::some(Method {
                    request: format!(".{}", ROUTINE_START_REQUEST),
                    response: format!(".{}", ROUTINE_START_RESPONSE),
                    ..Default::default()
                }),
                stop: MessageField::some(Method {
                    request: format!(".{}", ROUTINE_STOP_REQUEST),
                    response: format!(".{}", ROUTINE_STOP_RESPONSE),
                    ..Default::default()
                }),
                result: MessageField::some(Method {
                    request: format!(".{}", ROUTINE_RESULT_REQUEST),
                    response: format!(".{}", ROUTINE_RESULT_RESPONSE),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[googletest::test]
    fn translate_routine_start_request_to_uds() {
        let translator = create_diagnostics_translator(
            service_fqin("ServiceBundleName"),
            routine_declaration("routine_id"),
        );

        assert_that!(
            translator.translate_from_protobuf_to_uds(
                &service_fqin("ServiceBundleName"),
                EntityId::Routine {
                    id: "routine_id".to_string(),
                    method: RoutineMethod::Start,
                    message: MethodMessage::Request,
                },
                &MESSAGE_BYTES[..]
            ),
            matches_pattern!(Ok(eq(uds_bytes_for_message(ROUTINE_START_REQUEST))))
        );
    }

    #[googletest::test]
    fn translate_routine_start_request_to_protobuf() {
        let translator = create_diagnostics_translator(
            service_fqin("ServiceBundleName"),
            routine_declaration("routine_id"),
        );

        assert_that!(
            translator.translate_from_uds_to_protobuf(
                &service_fqin("ServiceBundleName"),
                EntityId::Routine {
                    id: "routine_id".to_string(),
                    method: RoutineMethod::Start,
                    message: MethodMessage::Request,
                },
                &MESSAGE_BYTES[..]
            ),
            matches_pattern!(Ok(eq(proto_bytes_for_message(ROUTINE_START_REQUEST))))
        );
    }

    #[googletest::test]
    fn translate_routine_start_response_to_uds() {
        let translator = create_diagnostics_translator(
            service_fqin("ServiceBundleName"),
            routine_declaration("routine_id"),
        );

        assert_that!(
            translator.translate_from_protobuf_to_uds(
                &service_fqin("ServiceBundleName"),
                EntityId::Routine {
                    id: "routine_id".to_string(),
                    method: RoutineMethod::Start,
                    message: MethodMessage::Response,
                },
                &MESSAGE_BYTES[..]
            ),
            matches_pattern!(Ok(eq(uds_bytes_for_message(ROUTINE_START_RESPONSE))))
        );
    }

    #[googletest::test]
    fn translate_routine_start_response_to_protobuf() {
        let translator = create_diagnostics_translator(
            service_fqin("ServiceBundleName"),
            routine_declaration("routine_id"),
        );

        assert_that!(
            translator.translate_from_uds_to_protobuf(
                &service_fqin("ServiceBundleName"),
                EntityId::Routine {
                    id: "routine_id".to_string(),
                    method: RoutineMethod::Start,
                    message: MethodMessage::Response,
                },
                &MESSAGE_BYTES[..]
            ),
            matches_pattern!(Ok(eq(proto_bytes_for_message(ROUTINE_START_RESPONSE))))
        );
    }

    #[googletest::test]
    fn translate_routine_stop_request_to_uds() {
        let translator = create_diagnostics_translator(
            service_fqin("ServiceBundleName"),
            routine_declaration("routine_id"),
        );

        assert_that!(
            translator.translate_from_protobuf_to_uds(
                &service_fqin("ServiceBundleName"),
                EntityId::Routine {
                    id: "routine_id".to_string(),
                    method: RoutineMethod::Stop,
                    message: MethodMessage::Request,
                },
                &MESSAGE_BYTES[..]
            ),
            matches_pattern!(Ok(eq(uds_bytes_for_message(ROUTINE_STOP_REQUEST))))
        );
    }

    #[googletest::test]
    fn translate_routine_stop_request_to_protobuf() {
        let translator = create_diagnostics_translator(
            service_fqin("ServiceBundleName"),
            routine_declaration("routine_id"),
        );

        assert_that!(
            translator.translate_from_uds_to_protobuf(
                &service_fqin("ServiceBundleName"),
                EntityId::Routine {
                    id: "routine_id".to_string(),
                    method: RoutineMethod::Stop,
                    message: MethodMessage::Request,
                },
                &MESSAGE_BYTES[..]
            ),
            matches_pattern!(Ok(eq(proto_bytes_for_message(ROUTINE_STOP_REQUEST))))
        );
    }

    #[googletest::test]
    fn translate_routine_stop_response_to_uds() {
        let translator = create_diagnostics_translator(
            service_fqin("ServiceBundleName"),
            routine_declaration("routine_id"),
        );

        assert_that!(
            translator.translate_from_protobuf_to_uds(
                &service_fqin("ServiceBundleName"),
                EntityId::Routine {
                    id: "routine_id".to_string(),
                    method: RoutineMethod::Stop,
                    message: MethodMessage::Response,
                },
                &MESSAGE_BYTES[..]
            ),
            matches_pattern!(Ok(eq(uds_bytes_for_message(ROUTINE_STOP_RESPONSE))))
        );
    }

    #[googletest::test]
    fn translate_routine_stop_response_to_protobuf() {
        let translator = create_diagnostics_translator(
            service_fqin("ServiceBundleName"),
            routine_declaration("routine_id"),
        );

        assert_that!(
            translator.translate_from_uds_to_protobuf(
                &service_fqin("ServiceBundleName"),
                EntityId::Routine {
                    id: "routine_id".to_string(),
                    method: RoutineMethod::Stop,
                    message: MethodMessage::Response,
                },
                &MESSAGE_BYTES[..]
            ),
            matches_pattern!(Ok(eq(proto_bytes_for_message(ROUTINE_STOP_RESPONSE))))
        );
    }

    #[googletest::test]
    fn translate_routine_result_request_to_uds() {
        let translator = create_diagnostics_translator(
            service_fqin("ServiceBundleName"),
            routine_declaration("routine_id"),
        );

        assert_that!(
            translator.translate_from_protobuf_to_uds(
                &service_fqin("ServiceBundleName"),
                EntityId::Routine {
                    id: "routine_id".to_string(),
                    method: RoutineMethod::RequestResult,
                    message: MethodMessage::Request,
                },
                &MESSAGE_BYTES[..]
            ),
            matches_pattern!(Ok(eq(uds_bytes_for_message(ROUTINE_RESULT_REQUEST))))
        );
    }

    #[googletest::test]
    fn translate_routine_result_request_to_protobuf() {
        let translator = create_diagnostics_translator(
            service_fqin("ServiceBundleName"),
            routine_declaration("routine_id"),
        );

        assert_that!(
            translator.translate_from_uds_to_protobuf(
                &service_fqin("ServiceBundleName"),
                EntityId::Routine {
                    id: "routine_id".to_string(),
                    method: RoutineMethod::RequestResult,
                    message: MethodMessage::Request,
                },
                &MESSAGE_BYTES[..]
            ),
            matches_pattern!(Ok(eq(proto_bytes_for_message(ROUTINE_RESULT_REQUEST))))
        );
    }

    #[googletest::test]
    fn translate_routine_result_response_to_uds() {
        let translator = create_diagnostics_translator(
            service_fqin("ServiceBundleName"),
            routine_declaration("routine_id"),
        );

        assert_that!(
            translator.translate_from_protobuf_to_uds(
                &service_fqin("ServiceBundleName"),
                EntityId::Routine {
                    id: "routine_id".to_string(),
                    method: RoutineMethod::RequestResult,
                    message: MethodMessage::Response,
                },
                &MESSAGE_BYTES[..]
            ),
            matches_pattern!(Ok(eq(uds_bytes_for_message(ROUTINE_RESULT_RESPONSE))))
        );
    }

    #[googletest::test]
    fn translate_routine_result_response_to_protobuf() {
        let translator = create_diagnostics_translator(
            service_fqin("ServiceBundleName"),
            routine_declaration("routine_id"),
        );

        assert_that!(
            translator.translate_from_uds_to_protobuf(
                &service_fqin("ServiceBundleName"),
                EntityId::Routine {
                    id: "routine_id".to_string(),
                    method: RoutineMethod::RequestResult,
                    message: MethodMessage::Response,
                },
                &MESSAGE_BYTES[..]
            ),
            matches_pattern!(Ok(eq(proto_bytes_for_message(ROUTINE_RESULT_RESPONSE))))
        );
    }

    fn event_declaration(event_id: &str) -> DiagnosticsDeclaration {
        DiagnosticsDeclaration {
            event: vec![Event {
                id: event_id.to_string(),
                extended_data_message_name: format!(".{}", EVENT_EXTENDED_DATA),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[googletest::test]
    fn translate_event_data_to_uds() {
        let translator = create_diagnostics_translator(
            service_fqin("ServiceBundleName"),
            event_declaration("event_id"),
        );

        assert_that!(
            translator.translate_from_protobuf_to_uds(
                &service_fqin("ServiceBundleName"),
                EntityId::EventExtendedData("event_id".to_string()),
                &MESSAGE_BYTES[..]
            ),
            matches_pattern!(Ok(eq(uds_bytes_for_message(EVENT_EXTENDED_DATA))))
        );
    }

    #[googletest::test]
    fn translate_event_data_to_protobuf() {
        let translator = create_diagnostics_translator(
            service_fqin("ServiceBundleName"),
            event_declaration("event_id"),
        );

        assert_that!(
            translator.translate_from_uds_to_protobuf(
                &service_fqin("ServiceBundleName"),
                EntityId::EventExtendedData("event_id".to_string()),
                &MESSAGE_BYTES[..]
            ),
            matches_pattern!(Ok(eq(proto_bytes_for_message(EVENT_EXTENDED_DATA))))
        );
    }

    #[googletest::test]
    fn translate_service_declaration_missing() {
        let translator = create_diagnostics_translator(
            service_fqin("NotServiceBundleName"),
            DiagnosticsDeclaration { ..Default::default() },
        );

        assert_that!(
            translator.translate_from_uds_to_protobuf(
                &service_fqin("ServiceBundleName"),
                EntityId::DataItem("data_item_id".to_string()),
                &MESSAGE_BYTES[..]
            ),
            matches_pattern!(Err(matches_pattern!(MissingServiceBundleDeclarationError(eq(
                service_fqin("ServiceBundleName").to_string()
            )))))
        );
    }

    #[googletest::test]
    fn translate_entity_declaration_missing() {
        let translator = create_diagnostics_translator(
            service_fqin("ServiceBundleName"),
            DiagnosticsDeclaration { ..Default::default() },
        );

        assert_that!(
            translator.translate_from_uds_to_protobuf(
                &service_fqin("ServiceBundleName"),
                EntityId::DataItem("data_item_id".to_string()),
                &MESSAGE_BYTES[..]
            ),
            matches_pattern!(Err(matches_pattern!(MissingEntityDeclarationError(
                eq(service_fqin("ServiceBundleName").to_string()),
                matches_pattern!(EntityId::DataItem(eq("data_item_id".to_string())))
            ))))
        );
    }

    fn service_fqin(bundle_name: &str) -> ServiceFqin {
        ServiceFqin::from_str(&format!("vm:com.package.{bundle_name}/instance")).unwrap()
    }
}
