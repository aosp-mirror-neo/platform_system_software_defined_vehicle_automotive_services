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
use diagnostics_v1_api::response_code::ResponseCode::{
    INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT, POSITIVE_RESPONSE, SUB_FUNCTION_NOT_SUPPORTED,
};
use diagnostics_v1_api::routine_control_service::RoutineControlResponse;
use protobuf::Message;
use std::future::Future;

/// Typed routine response.
#[derive(Clone, Default)]
pub struct RoutineResponse<T: Message> {
    /// See com.android.sdv.diagnostics.routine_control.RoutineControlResponse::routine_info.
    pub routine_info: i8,
    /// See com.android.sdv.diagnostics.routine_control.RoutineControlResponse::response.
    pub response: T,
}

/// Type alias for typed routine handler method responses.
pub type RoutineResult<T> = DiagnosticsResult<RoutineResponse<T>>;

/// Typed trait that corresponds to RoutineControl server
///
/// See com.android.sdv.diagnostics.routine_control.RoutineControl service.
#[async_trait::async_trait]
pub trait RoutineHandler<
    StartRequest: Message,
    StartResponse: Message,
    StopRequest: Message,
    StopResponse: Message,
    ResultRequest: Message,
    ResultResponse: Message,
>: Send + Sync
{
    /// See com.android.sdv.diagnostics.routine_control.RoutineControl::Start.
    async fn start(&self, req: StartRequest) -> RoutineResult<StartResponse>;

    /// Untyped version of `start`.
    async fn start_untyped(&self, request_bytes: &[u8]) -> RoutineControlResponse {
        handle_routine_request(|request| self.start(request), request_bytes).await
    }

    /// See com.android.sdv.diagnostics.routine_control.RoutineControl::Stop.
    async fn stop(&self, _req: StopRequest) -> RoutineResult<StopResponse> {
        RoutineResult::Err(SUB_FUNCTION_NOT_SUPPORTED)
    }

    /// Untyped version of `stop`.
    async fn stop_untyped(&self, request_bytes: &[u8]) -> RoutineControlResponse {
        handle_routine_request(|request| self.stop(request), request_bytes).await
    }

    /// See com.android.sdv.diagnostics.routine_control.RoutineControl::RequestResult.
    async fn request_result(&self, _req: ResultRequest) -> RoutineResult<ResultResponse> {
        RoutineResult::Err(SUB_FUNCTION_NOT_SUPPORTED)
    }

    /// Untyped version of `request_result`.
    async fn request_result_untyped(&self, request_bytes: &[u8]) -> RoutineControlResponse {
        handle_routine_request(|request| self.request_result(request), request_bytes).await
    }
}

async fn handle_routine_request<Request: Message, Response: Message, F, Fut>(
    handler: F,
    request_bytes: &[u8],
) -> RoutineControlResponse
where
    F: Fn(Request) -> Fut,
    Fut: Future<Output = RoutineResult<Response>>,
{
    let parsed_request = Request::parse_from_bytes(request_bytes);

    match parsed_request {
        Ok(request) => {
            let result = handler(request).await;

            match result {
                Ok(result) => {
                    let response_bytes = result.response.write_to_bytes();

                    response_bytes
                        .map(|response_bytes| RoutineControlResponse {
                            routine_info: result.routine_info.into(),
                            response: response_bytes,
                            response_code: POSITIVE_RESPONSE.into(),
                            ..Default::default()
                        })
                        .unwrap_or_else(|_| RoutineControlResponse {
                            response_code: INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT.into(),
                            ..Default::default()
                        })
                }
                Err(response_code) => RoutineControlResponse {
                    response_code: response_code.into(),
                    ..Default::default()
                },
            }
        }
        _ => RoutineControlResponse {
            response_code: INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT.into(),
            ..Default::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use crate::routine::{RoutineHandler, RoutineResponse, RoutineResult};
    use crate::test::test_messages::UnserializableMessage;
    use diagnostics_v1_api::event::Event as TestProto;
    use diagnostics_v1_api::event::Status;
    use diagnostics_v1_api::response_code::ResponseCode;
    use diagnostics_v1_api::routine_control_service::RoutineControlResponse;
    use protobuf::Message;

    struct TestRoutineHandler {
        response: RoutineResult<TestProto>,
    }

    #[async_trait::async_trait]
    impl RoutineHandler<TestProto, TestProto, TestProto, TestProto, TestProto, TestProto>
        for TestRoutineHandler
    {
        async fn start(&self, _: TestProto) -> RoutineResult<TestProto> {
            self.response.clone()
        }

        async fn stop(&self, _: TestProto) -> RoutineResult<TestProto> {
            self.response.clone()
        }

        async fn request_result(&self, _: TestProto) -> RoutineResult<TestProto> {
            self.response.clone()
        }
    }

    struct UnserializableRoutineHandler;

    #[async_trait::async_trait]
    impl
        RoutineHandler<
            TestProto,
            UnserializableMessage,
            TestProto,
            UnserializableMessage,
            TestProto,
            UnserializableMessage,
        > for UnserializableRoutineHandler
    {
        async fn start(&self, _: TestProto) -> RoutineResult<UnserializableMessage> {
            Ok(RoutineResponse { response: UnserializableMessage::default(), routine_info: 0 })
        }

        async fn stop(&self, _: TestProto) -> RoutineResult<UnserializableMessage> {
            Ok(RoutineResponse { response: UnserializableMessage::default(), routine_info: 0 })
        }

        async fn request_result(&self, _: TestProto) -> RoutineResult<UnserializableMessage> {
            Ok(RoutineResponse { response: UnserializableMessage::default(), routine_info: 0 })
        }
    }

    #[tokio::test]
    async fn test_routine_start_untyped_ok() {
        let message = TestProto { status: Status::PASS.into(), ..Default::default() };

        let handler = TestRoutineHandler {
            response: Ok(RoutineResponse { response: message.clone(), routine_info: 0 }),
        };

        assert_eq!(
            handler.start_untyped(&message.write_to_bytes().unwrap()).await,
            RoutineControlResponse {
                response: message.write_to_bytes().unwrap(),
                ..Default::default()
            }
        );
    }

    #[tokio::test]
    async fn test_routine_start_untyped_err() {
        let message = TestProto { status: Status::PASS.into(), ..Default::default() };

        let handler = TestRoutineHandler { response: Err(ResponseCode::GENERAL_REJECT) };

        assert_eq!(
            handler.start_untyped(&message.write_to_bytes().unwrap()).await,
            RoutineControlResponse {
                response_code: ResponseCode::GENERAL_REJECT.into(),
                ..Default::default()
            }
        );
    }

    #[tokio::test]
    async fn test_routine_start_untyped_deserialization_format_err() {
        let invalid_message_bytes = vec![2, 1];

        let handler = TestRoutineHandler { response: Err(ResponseCode::GENERAL_REJECT) };

        assert_eq!(
            handler.start_untyped(&invalid_message_bytes).await,
            RoutineControlResponse {
                response_code: ResponseCode::INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT.into(),
                ..Default::default()
            }
        );
    }

    #[tokio::test]
    async fn test_routine_start_untyped_serialization_format_err() {
        let message = TestProto { status: Status::PASS.into(), ..Default::default() };
        let handler = UnserializableRoutineHandler;

        assert_eq!(
            handler.start_untyped(&message.write_to_bytes().unwrap()).await,
            RoutineControlResponse {
                response_code: ResponseCode::INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT.into(),
                ..Default::default()
            }
        );
    }

    #[tokio::test]
    async fn test_routine_stop_untyped_ok() {
        let message = TestProto { status: Status::PASS.into(), ..Default::default() };

        let handler = TestRoutineHandler {
            response: Ok(RoutineResponse { response: message.clone(), routine_info: 0 }),
        };

        assert_eq!(
            handler.stop_untyped(&message.write_to_bytes().unwrap()).await,
            RoutineControlResponse {
                response: message.write_to_bytes().unwrap(),
                ..Default::default()
            }
        );
    }

    #[tokio::test]
    async fn test_routine_stop_untyped_err() {
        let message = TestProto { status: Status::PASS.into(), ..Default::default() };

        let handler = TestRoutineHandler { response: Err(ResponseCode::GENERAL_REJECT) };

        assert_eq!(
            handler.stop_untyped(&message.write_to_bytes().unwrap()).await,
            RoutineControlResponse {
                response_code: ResponseCode::GENERAL_REJECT.into(),
                ..Default::default()
            }
        );
    }

    #[tokio::test]
    async fn test_routine_stop_untyped_deserialization_format_err() {
        let invalid_message_bytes = vec![2, 1];

        let handler = TestRoutineHandler { response: Err(ResponseCode::GENERAL_REJECT) };

        assert_eq!(
            handler.stop_untyped(&invalid_message_bytes).await,
            RoutineControlResponse {
                response_code: ResponseCode::INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT.into(),
                ..Default::default()
            }
        );
    }

    #[tokio::test]
    async fn test_routine_stop_untyped_serialization_format_err() {
        let message = TestProto { status: Status::PASS.into(), ..Default::default() };
        let handler = UnserializableRoutineHandler;

        assert_eq!(
            handler.stop_untyped(&message.write_to_bytes().unwrap()).await,
            RoutineControlResponse {
                response_code: ResponseCode::INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT.into(),
                ..Default::default()
            }
        );
    }

    #[tokio::test]
    async fn test_routine_request_result_untyped_ok() {
        let message = TestProto { status: Status::PASS.into(), ..Default::default() };

        let handler = TestRoutineHandler {
            response: Ok(RoutineResponse { response: message.clone(), routine_info: 0 }),
        };

        assert_eq!(
            handler.request_result_untyped(&message.write_to_bytes().unwrap()).await,
            RoutineControlResponse {
                response: message.write_to_bytes().unwrap(),
                ..Default::default()
            }
        );
    }

    #[tokio::test]
    async fn test_routine_request_result_untyped_err() {
        let message = TestProto { status: Status::PASS.into(), ..Default::default() };

        let handler = TestRoutineHandler { response: Err(ResponseCode::GENERAL_REJECT) };

        assert_eq!(
            handler.request_result_untyped(&message.write_to_bytes().unwrap()).await,
            RoutineControlResponse {
                response_code: ResponseCode::GENERAL_REJECT.into(),
                ..Default::default()
            }
        );
    }

    #[tokio::test]
    async fn test_routine_request_result_untyped_deserialization_format_err() {
        let invalid_message_bytes = vec![2, 1];

        let handler = TestRoutineHandler { response: Err(ResponseCode::GENERAL_REJECT) };

        assert_eq!(
            handler.request_result_untyped(&invalid_message_bytes).await,
            RoutineControlResponse {
                response_code: ResponseCode::INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT.into(),
                ..Default::default()
            }
        );
    }

    #[tokio::test]
    async fn test_routine_request_result_untyped_serialization_format_err() {
        let message = TestProto { status: Status::PASS.into(), ..Default::default() };
        let handler = UnserializableRoutineHandler;

        assert_eq!(
            handler.request_result_untyped(&message.write_to_bytes().unwrap()).await,
            RoutineControlResponse {
                response_code: ResponseCode::INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT.into(),
                ..Default::default()
            }
        );
    }
}
