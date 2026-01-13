/*
 * Copyright (C) 2026 The Android Open Source Project
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use crate::DiagnosticsResult;
use diagnostics_v1_api::io_control_service::IoControlResponse;
use diagnostics_v1_api::response_code::ResponseCode::INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT;
use diagnostics_v1_api::response_code::ResponseCode::POSITIVE_RESPONSE;
use protobuf::MessageFull;

/// Typed trait that corresponds to IoControl server
///
/// See com.sdv.google.diagnostics.io_control.IoControl service.
#[async_trait::async_trait]
pub trait IoControlHandler<T: MessageFull>: Send + Sync {
    /// See com.sdv.google.diagnostics.io_control.IoControl::ReturnControlToEcu.
    async fn return_control_to_ecu(&self, mask: &[u8]) -> DiagnosticsResult<T>;

    /// Untyped version of `return_control_to_ecu`.
    async fn return_control_to_ecu_untyped(&self, mask: &[u8]) -> IoControlResponse {
        to_io_control_response(self.return_control_to_ecu(mask).await)
    }

    /// See com.sdv.google.diagnostics.io_control.IoControl::ResetToDefault.
    async fn reset_to_default(&self, mask: &[u8]) -> DiagnosticsResult<T>;

    /// Untyped version of `stop`.
    async fn reset_to_default_untyped(&self, mask: &[u8]) -> IoControlResponse {
        to_io_control_response(self.reset_to_default(mask).await)
    }

    /// See com.sdv.google.diagnostics.io_control.IoControl::FreezeCurrentState.
    async fn freeze_current_state(&self, mask: &[u8]) -> DiagnosticsResult<T>;

    /// Untyped version of `request_result`.
    async fn freeze_current_state_untyped(&self, mask: &[u8]) -> IoControlResponse {
        to_io_control_response(self.freeze_current_state(mask).await)
    }

    /// See com.sdv.google.diagnostics.io_control.IoControl::ShortTermAdjustment.
    async fn short_term_adjustment(&self, adjusted_value: T, mask: &[u8]) -> DiagnosticsResult<T>;

    /// Untyped version of `short_term_adjustment`.
    async fn short_term_adjustment_untyped(
        &self,
        adjusted_value_bytes: &[u8],
        mask: &[u8],
    ) -> IoControlResponse {
        let parsed_value = T::parse_from_bytes(adjusted_value_bytes);

        match parsed_value {
            Ok(adjusted_value) => {
                to_io_control_response(self.short_term_adjustment(adjusted_value, mask).await)
            }
            _ => IoControlResponse {
                response_code: INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT.into(),
                ..Default::default()
            },
        }
    }
}

fn to_io_control_response<T: MessageFull>(result: DiagnosticsResult<T>) -> IoControlResponse {
    match result {
        Ok(value) => {
            let value_bytes = value.write_to_bytes();

            value_bytes
                .map(|value_bytes| IoControlResponse {
                    value: value_bytes,
                    response_code: POSITIVE_RESPONSE.into(),
                    ..Default::default()
                })
                .unwrap_or_else(|_| IoControlResponse {
                    response_code: INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT.into(),
                    ..Default::default()
                })
        }
        Err(response_code) => {
            IoControlResponse { response_code: response_code.into(), ..Default::default() }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::io_control::IoControlHandler;
    use crate::test::test_messages::UnserializableMessage;
    use crate::DiagnosticsResult;
    use diagnostics_v1_api::event::Event as TestProto;
    use diagnostics_v1_api::event::Status;
    use diagnostics_v1_api::io_control_service::IoControlResponse;
    use diagnostics_v1_api::response_code::ResponseCode;
    use protobuf::Message;

    const MASK: &[u8] = &[1];

    struct TestIoControlHandler {
        response: DiagnosticsResult<TestProto>,
    }

    #[async_trait::async_trait]
    impl IoControlHandler<TestProto> for TestIoControlHandler {
        async fn return_control_to_ecu(&self, _mask: &[u8]) -> DiagnosticsResult<TestProto> {
            self.response.clone()
        }

        async fn reset_to_default(&self, _mask: &[u8]) -> DiagnosticsResult<TestProto> {
            self.response.clone()
        }

        async fn freeze_current_state(&self, _mask: &[u8]) -> DiagnosticsResult<TestProto> {
            self.response.clone()
        }

        async fn short_term_adjustment(
            &self,
            _adjusted_value: TestProto,
            _mask: &[u8],
        ) -> DiagnosticsResult<TestProto> {
            self.response.clone()
        }
    }

    struct UnserializableIoControlHandler;

    #[async_trait::async_trait]
    impl IoControlHandler<UnserializableMessage> for UnserializableIoControlHandler {
        async fn return_control_to_ecu(
            &self,
            _mask: &[u8],
        ) -> DiagnosticsResult<UnserializableMessage> {
            Ok(UnserializableMessage::default())
        }

        async fn reset_to_default(&self, _mask: &[u8]) -> DiagnosticsResult<UnserializableMessage> {
            Ok(UnserializableMessage::default())
        }

        async fn freeze_current_state(
            &self,
            _mask: &[u8],
        ) -> DiagnosticsResult<UnserializableMessage> {
            Ok(UnserializableMessage::default())
        }

        async fn short_term_adjustment(
            &self,
            _adjusted_value: UnserializableMessage,
            _mask: &[u8],
        ) -> DiagnosticsResult<UnserializableMessage> {
            Ok(UnserializableMessage::default())
        }
    }

    #[tokio::test]
    async fn test_return_control_to_ecu_untyped_ok() {
        let message = TestProto { status: Status::PASS.into(), ..Default::default() };

        let handler = TestIoControlHandler { response: Ok(message.clone()) };

        assert_eq!(
            handler.return_control_to_ecu_untyped(MASK).await,
            IoControlResponse { value: message.write_to_bytes().unwrap(), ..Default::default() }
        );
    }

    #[tokio::test]
    async fn test_return_control_to_ecu_untyped_err() {
        let handler = TestIoControlHandler { response: Err(ResponseCode::GENERAL_REJECT) };

        assert_eq!(
            handler.return_control_to_ecu_untyped(MASK).await,
            IoControlResponse {
                response_code: ResponseCode::GENERAL_REJECT.into(),
                ..Default::default()
            }
        );
    }

    #[tokio::test]
    async fn test_return_control_to_ecu_untyped_format_err() {
        let handler = UnserializableIoControlHandler;

        assert_eq!(
            handler.return_control_to_ecu_untyped(MASK).await,
            IoControlResponse {
                response_code: ResponseCode::INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT.into(),
                ..Default::default()
            }
        );
    }

    #[tokio::test]
    async fn test_reset_to_default_untyped_ok() {
        let message = TestProto { status: Status::PASS.into(), ..Default::default() };

        let handler = TestIoControlHandler { response: Ok(message.clone()) };

        assert_eq!(
            handler.reset_to_default_untyped(MASK).await,
            IoControlResponse { value: message.write_to_bytes().unwrap(), ..Default::default() }
        );
    }

    #[tokio::test]
    async fn test_reset_to_default_untyped_err() {
        let handler = TestIoControlHandler { response: Err(ResponseCode::GENERAL_REJECT) };

        assert_eq!(
            handler.reset_to_default_untyped(MASK).await,
            IoControlResponse {
                response_code: ResponseCode::GENERAL_REJECT.into(),
                ..Default::default()
            }
        );
    }

    #[tokio::test]
    async fn test_reset_to_default_untyped_format_err() {
        let handler = UnserializableIoControlHandler;

        assert_eq!(
            handler.reset_to_default_untyped(MASK).await,
            IoControlResponse {
                response_code: ResponseCode::INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT.into(),
                ..Default::default()
            }
        );
    }

    #[tokio::test]
    async fn test_freeze_current_state_untyped_ok() {
        let message = TestProto { status: Status::PASS.into(), ..Default::default() };

        let handler = TestIoControlHandler { response: Ok(message.clone()) };

        assert_eq!(
            handler.freeze_current_state_untyped(MASK).await,
            IoControlResponse { value: message.write_to_bytes().unwrap(), ..Default::default() }
        );
    }

    #[tokio::test]
    async fn test_freeze_current_state_untyped_err() {
        let handler = TestIoControlHandler { response: Err(ResponseCode::GENERAL_REJECT) };

        assert_eq!(
            handler.freeze_current_state_untyped(MASK).await,
            IoControlResponse {
                response_code: ResponseCode::GENERAL_REJECT.into(),
                ..Default::default()
            }
        );
    }

    #[tokio::test]
    async fn test_freeze_current_state_untyped_format_err() {
        let handler = UnserializableIoControlHandler;

        assert_eq!(
            handler.freeze_current_state_untyped(MASK).await,
            IoControlResponse {
                response_code: ResponseCode::INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT.into(),
                ..Default::default()
            }
        );
    }

    #[tokio::test]
    async fn test_short_term_adjustment_untyped_ok() {
        let message = TestProto { status: Status::PASS.into(), ..Default::default() };

        let handler = TestIoControlHandler { response: Ok(message.clone()) };

        assert_eq!(
            handler.short_term_adjustment_untyped(&message.write_to_bytes().unwrap(), MASK).await,
            IoControlResponse { value: message.write_to_bytes().unwrap(), ..Default::default() }
        );
    }

    #[tokio::test]
    async fn test_short_term_adjustment_untyped_err() {
        let message = TestProto { status: Status::PASS.into(), ..Default::default() };

        let handler = TestIoControlHandler { response: Err(ResponseCode::GENERAL_REJECT) };

        assert_eq!(
            handler.short_term_adjustment_untyped(&message.write_to_bytes().unwrap(), MASK).await,
            IoControlResponse {
                response_code: ResponseCode::GENERAL_REJECT.into(),
                ..Default::default()
            }
        );
    }

    #[tokio::test]
    async fn test_short_term_adjustment_untyped_format_err() {
        let invalid_message_bytes = vec![2, 1];

        let handler = TestIoControlHandler { response: Err(ResponseCode::GENERAL_REJECT) };

        assert_eq!(
            handler.short_term_adjustment_untyped(&invalid_message_bytes, MASK).await,
            IoControlResponse {
                response_code: ResponseCode::INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT.into(),
                ..Default::default()
            }
        );
    }
}
