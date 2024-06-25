// Copyright 2024 Google LLC

use diagnostics_api::fault_listener_service::OnFaultStatusChangeResponse;
use diagnostics_api::response_code::ResponseCode::INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT;
use diagnostics_api::response_code::ResponseCode::POSITIVE_RESPONSE;
use protobuf::MessageFull;

/// Typed trait that corresponds to FaultListener server
///
/// See sdv.diagnostics.fault.FaultListener service.
pub trait FaultListenerHandler<T: MessageFull>: Send + Sync {
    /// See sdv.diagnostics.fault.FaultListener::OnFaultStatusChange.
    fn on_fault_status_change(&self, status: u8, extended_data: Option<T>);

    /// Untyped version of `on_fault_status_change`.
    fn on_fault_status_change_untyped(
        &self,
        status: u8,
        extended_data_bytes: Option<&[u8]>,
    ) -> OnFaultStatusChangeResponse {
        extended_data_bytes
            .map(T::parse_from_bytes)
            .transpose()
            .map(|extended_data| {
                self.on_fault_status_change(status, extended_data);
                OnFaultStatusChangeResponse {
                    response_code: POSITIVE_RESPONSE.into(),
                    ..Default::default()
                }
            })
            .unwrap_or_else(|_| OnFaultStatusChangeResponse {
                response_code: INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT.into(),
                ..Default::default()
            })
    }
}

#[cfg(test)]
mod tests {
    use crate::fault_listener::FaultListenerHandler;
    use diagnostics_api::event::Event as TestProto;
    use diagnostics_api::event::Status;
    use diagnostics_api::fault_listener_service::OnFaultStatusChangeResponse;
    use diagnostics_api::response_code::ResponseCode;
    use protobuf::Message;

    const STATUS: u8 = 1;

    struct TestFaultListenerHandler {}

    impl FaultListenerHandler<TestProto> for TestFaultListenerHandler {
        fn on_fault_status_change(&self, _: u8, _: Option<TestProto>) {}
    }

    #[test]
    fn test_on_fault_status_change_untyped() {
        let message_bytes = TestProto { status: Status::PASS.into(), ..Default::default() }
            .write_to_bytes()
            .unwrap();

        let handler = TestFaultListenerHandler {};

        assert_eq!(
            handler.on_fault_status_change_untyped(STATUS, Some(&message_bytes)),
            OnFaultStatusChangeResponse {
                response_code: ResponseCode::POSITIVE_RESPONSE.into(),
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_on_fault_status_change_untyped_format_err() {
        let invalid_message_bytes = vec![2, 1];

        let handler = TestFaultListenerHandler {};

        assert_eq!(
            handler.on_fault_status_change_untyped(STATUS, Some(&invalid_message_bytes)),
            OnFaultStatusChangeResponse {
                response_code: ResponseCode::INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT.into(),
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_on_fault_status_change_untyped_empty_extended_data() {
        let handler = TestFaultListenerHandler {};

        assert_eq!(
            handler.on_fault_status_change_untyped(STATUS, None),
            OnFaultStatusChangeResponse {
                response_code: ResponseCode::POSITIVE_RESPONSE.into(),
                ..Default::default()
            }
        );
    }
}
