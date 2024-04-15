// Copyright 2024 Google LLC

use diagnostics_api::negative_response_code::NegativeResponseCode::INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT;
use diagnostics_api::negative_response_code::{EmptyResponse, NegativeResponseCode};
use protobuf::MessageFull;

/// Typed trait that corresponds to FaultListener server
///
/// See sdv.diagnostics.fault.FaultListener service.
pub trait FaultListenerHandler<T: MessageFull>: Send + Sync {
    /// See sdv.diagnostics.fault.FaultListener::OnFaultStatusChange.
    fn on_fault_status_change(&self, status: u8, extended_data: Option<T>) -> NegativeResponseCode;

    /// Untyped version of `on_fault_status_change`.
    fn on_fault_status_change_untyped(
        &self,
        status: u8,
        extended_data_bytes: Option<&[u8]>,
    ) -> EmptyResponse {
        extended_data_bytes
            .map(T::parse_from_bytes)
            .transpose()
            .map(|extended_data| {
                let result = self.on_fault_status_change(status, extended_data);
                EmptyResponse { error_code: result.into(), ..Default::default() }
            })
            .unwrap_or_else(|_| EmptyResponse {
                error_code: INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT.into(),
                ..Default::default()
            })
    }
}

#[cfg(test)]
mod tests {
    use crate::fault_listener::FaultListenerHandler;
    use diagnostics_api::event::Event as TestProto;
    use diagnostics_api::event::Status;
    use diagnostics_api::negative_response_code::{EmptyResponse, NegativeResponseCode};
    use protobuf::Message;

    const STATUS: u8 = 1;

    struct TestFaultListenerHandler {
        response: NegativeResponseCode,
    }

    impl FaultListenerHandler<TestProto> for TestFaultListenerHandler {
        fn on_fault_status_change(&self, _: u8, _: Option<TestProto>) -> NegativeResponseCode {
            self.response
        }
    }

    #[test]
    fn test_on_fault_status_change_untyped_ok() {
        let message_bytes = TestProto { status: Status::PASS.into(), ..Default::default() }
            .write_to_bytes()
            .unwrap();

        let handler =
            TestFaultListenerHandler { response: NegativeResponseCode::POSITIVE_RESPONSE };

        assert_eq!(
            handler.on_fault_status_change_untyped(STATUS, Some(&message_bytes)),
            EmptyResponse { ..Default::default() }
        );
    }

    #[test]
    fn test_on_fault_status_change_untyped_err() {
        let message_bytes = TestProto { status: Status::PASS.into(), ..Default::default() }
            .write_to_bytes()
            .unwrap();

        let handler = TestFaultListenerHandler { response: NegativeResponseCode::GENERAL_REJECT };

        assert_eq!(
            handler.on_fault_status_change_untyped(STATUS, Some(&message_bytes)),
            EmptyResponse {
                error_code: NegativeResponseCode::GENERAL_REJECT.into(),
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_on_fault_status_change_untyped_format_err() {
        let invalid_message_bytes = vec![2, 1];

        let handler = TestFaultListenerHandler { response: NegativeResponseCode::GENERAL_REJECT };

        assert_eq!(
            handler.on_fault_status_change_untyped(STATUS, Some(&invalid_message_bytes)),
            EmptyResponse {
                error_code: NegativeResponseCode::INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT.into(),
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_on_fault_status_change_untyped_empty_extended_data() {
        let handler =
            TestFaultListenerHandler { response: NegativeResponseCode::POSITIVE_RESPONSE };

        assert_eq!(
            handler.on_fault_status_change_untyped(STATUS, None),
            EmptyResponse {
                error_code: NegativeResponseCode::POSITIVE_RESPONSE.into(),
                ..Default::default()
            }
        );
    }
}
