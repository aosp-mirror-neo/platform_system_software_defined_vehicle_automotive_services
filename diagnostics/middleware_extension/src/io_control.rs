// Copyright 2024 Google LLC

use crate::DiagnosticsResult;
use diagnostics_api::io_control::IOControlResponse as IoControlResponse;
use diagnostics_api::negative_response_code::NegativeResponseCode::INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT;
use diagnostics_api::negative_response_code::NegativeResponseCode::POSITIVE_RESPONSE;
use protobuf::MessageFull;

/// Typed trait that corresponds to IoControl server
///
/// See sdv.diagnostics.io_control.IoControl service.
pub trait IoControlHandler<T: MessageFull>: Send + Sync {
    /// See sdv.diagnostics.io_control.IoControl::ReturnControlToEcu.
    fn return_control_to_ecu(&self, mask: &[u8]) -> DiagnosticsResult<T>;

    /// Untyped version of `return_control_to_ecu`.
    fn return_control_to_ecu_untyped(&self, mask: &[u8]) -> IoControlResponse {
        to_io_control_response(self.return_control_to_ecu(mask))
    }

    /// See sdv.diagnostics.io_control.IoControl::ResetToDefault.
    fn reset_to_default(&self, mask: &[u8]) -> DiagnosticsResult<T>;

    /// Untyped version of `stop`.
    fn reset_to_default_untyped(&self, mask: &[u8]) -> IoControlResponse {
        to_io_control_response(self.reset_to_default(mask))
    }

    /// See sdv.diagnostics.io_control.IoControl::FreezeCurrentState.
    fn freeze_current_state(&self, mask: &[u8]) -> DiagnosticsResult<T>;

    /// Untyped version of `request_result`.
    fn freeze_current_state_untyped(&self, mask: &[u8]) -> IoControlResponse {
        to_io_control_response(self.freeze_current_state(mask))
    }

    /// See sdv.diagnostics.io_control.IoControl::ShortTermAdjustment.
    fn short_term_adjustment(&self, adjusted_value: T, mask: &[u8]) -> DiagnosticsResult<T>;

    /// Untyped version of `short_term_adjustment`.
    fn short_term_adjustment_untyped(
        &self,
        adjusted_value_bytes: &[u8],
        mask: &[u8],
    ) -> IoControlResponse {
        T::parse_from_bytes(adjusted_value_bytes)
            .map(|adjusted_value| {
                to_io_control_response(self.short_term_adjustment(adjusted_value, mask))
            })
            .unwrap_or_else(|_| IoControlResponse {
                error_code: INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT.into(),
                ..Default::default()
            })
    }
}

fn to_io_control_response<T: MessageFull>(result: DiagnosticsResult<T>) -> IoControlResponse {
    match result {
        Ok(value) => {
            let value_bytes = value.write_to_bytes();

            value_bytes
                .map(|value_bytes| IoControlResponse {
                    value: value_bytes,
                    error_code: POSITIVE_RESPONSE.into(),
                    ..Default::default()
                })
                .unwrap_or_else(|_| IoControlResponse {
                    error_code: INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT.into(),
                    ..Default::default()
                })
        }
        Err(error_code) => {
            IoControlResponse { error_code: error_code.into(), ..Default::default() }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::io_control::IoControlHandler;
    use crate::DiagnosticsResult;
    use diagnostics_api::event::Event as TestProto;
    use diagnostics_api::event::Status;
    use diagnostics_api::io_control::IOControlResponse as IoControlResponse;
    use diagnostics_api::negative_response_code::NegativeResponseCode;
    use protobuf::Message;

    const MASK: &[u8] = &[1];

    struct TestIoControlHandler {
        response: DiagnosticsResult<TestProto>,
    }

    impl IoControlHandler<TestProto> for TestIoControlHandler {
        fn return_control_to_ecu(&self, _mask: &[u8]) -> DiagnosticsResult<TestProto> {
            self.response.clone()
        }

        fn reset_to_default(&self, _mask: &[u8]) -> DiagnosticsResult<TestProto> {
            self.response.clone()
        }

        fn freeze_current_state(&self, _mask: &[u8]) -> DiagnosticsResult<TestProto> {
            self.response.clone()
        }

        fn short_term_adjustment(
            &self,
            _adjusted_value: TestProto,
            _mask: &[u8],
        ) -> DiagnosticsResult<TestProto> {
            self.response.clone()
        }
    }

    #[test]
    fn test_return_control_to_ecu_untyped_ok() {
        let message = TestProto { status: Status::PASS.into(), ..Default::default() };

        let handler = TestIoControlHandler { response: Ok(message.clone()) };

        assert_eq!(
            handler.return_control_to_ecu_untyped(MASK),
            IoControlResponse { value: message.write_to_bytes().unwrap(), ..Default::default() }
        );
    }

    #[test]
    fn test_return_control_to_ecu_untyped_err() {
        let handler = TestIoControlHandler { response: Err(NegativeResponseCode::GENERAL_REJECT) };

        assert_eq!(
            handler.return_control_to_ecu_untyped(MASK),
            IoControlResponse {
                error_code: NegativeResponseCode::GENERAL_REJECT.into(),
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_reset_to_default_untyped_ok() {
        let message = TestProto { status: Status::PASS.into(), ..Default::default() };

        let handler = TestIoControlHandler { response: Ok(message.clone()) };

        assert_eq!(
            handler.reset_to_default_untyped(MASK),
            IoControlResponse { value: message.write_to_bytes().unwrap(), ..Default::default() }
        );
    }

    #[test]
    fn test_reset_to_default_untyped_err() {
        let handler = TestIoControlHandler { response: Err(NegativeResponseCode::GENERAL_REJECT) };

        assert_eq!(
            handler.reset_to_default_untyped(MASK),
            IoControlResponse {
                error_code: NegativeResponseCode::GENERAL_REJECT.into(),
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_freeze_current_state_untyped_ok() {
        let message = TestProto { status: Status::PASS.into(), ..Default::default() };

        let handler = TestIoControlHandler { response: Ok(message.clone()) };

        assert_eq!(
            handler.freeze_current_state_untyped(MASK),
            IoControlResponse { value: message.write_to_bytes().unwrap(), ..Default::default() }
        );
    }

    #[test]
    fn test_freeze_current_state_untyped_err() {
        let handler = TestIoControlHandler { response: Err(NegativeResponseCode::GENERAL_REJECT) };

        assert_eq!(
            handler.freeze_current_state_untyped(MASK),
            IoControlResponse {
                error_code: NegativeResponseCode::GENERAL_REJECT.into(),
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_short_term_adjustment_untyped_ok() {
        let message = TestProto { status: Status::PASS.into(), ..Default::default() };

        let handler = TestIoControlHandler { response: Ok(message.clone()) };

        assert_eq!(
            handler.short_term_adjustment_untyped(&message.write_to_bytes().unwrap(), MASK),
            IoControlResponse { value: message.write_to_bytes().unwrap(), ..Default::default() }
        );
    }

    #[test]
    fn test_short_term_adjustment_untyped_err() {
        let message = TestProto { status: Status::PASS.into(), ..Default::default() };

        let handler = TestIoControlHandler { response: Err(NegativeResponseCode::GENERAL_REJECT) };

        assert_eq!(
            handler.short_term_adjustment_untyped(&message.write_to_bytes().unwrap(), MASK),
            IoControlResponse {
                error_code: NegativeResponseCode::GENERAL_REJECT.into(),
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_short_term_adjustment_untyped_format_err() {
        let invalid_message_bytes = vec![2, 1];

        let handler = TestIoControlHandler { response: Err(NegativeResponseCode::GENERAL_REJECT) };

        assert_eq!(
            handler.short_term_adjustment_untyped(&invalid_message_bytes, MASK),
            IoControlResponse {
                error_code: NegativeResponseCode::INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT.into(),
                ..Default::default()
            }
        );
    }
}
