// Copyright 2024 Google LLC

use crate::DiagnosticsResult;
use diagnostics_v1_api::data_item_service::{ReadDataItemResponse, WriteDataItemResponse};
use diagnostics_v1_api::response_code::ResponseCode;
use diagnostics_v1_api::response_code::ResponseCode::INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT;
use diagnostics_v1_api::response_code::ResponseCode::POSITIVE_RESPONSE;
use protobuf::MessageFull;

/// Typed trait that corresponds to read-only data item server
///
/// See com.sdv.google.diagnostics.data_item.DataItemService service.
#[async_trait::async_trait]
pub trait ReadOnlyDataItemHandler<T: MessageFull>: Send + Sync {
    /// Reads current data item value.
    async fn read(&self) -> DiagnosticsResult<T>;

    /// Reads current data item, converting it to the untyped ReadDataItemResponse.
    async fn read_untyped(&self) -> ReadDataItemResponse {
        to_read_response(self.read().await)
    }
}

fn to_read_response<T: MessageFull>(result: DiagnosticsResult<T>) -> ReadDataItemResponse {
    match result {
        Ok(data_item) => {
            let value_bytes = data_item.write_to_bytes();

            value_bytes
                .map(|value_bytes| ReadDataItemResponse {
                    value: value_bytes,
                    response_code: POSITIVE_RESPONSE.into(),
                    ..Default::default()
                })
                .unwrap_or_else(|_| ReadDataItemResponse {
                    response_code: INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT.into(),
                    ..Default::default()
                })
        }
        Err(response_code) => {
            ReadDataItemResponse { response_code: response_code.into(), ..Default::default() }
        }
    }
}

/// Typed trait that corresponds to writable data item server
///
/// See com.sdv.google.diagnostics.data_item.DataItemService service.
///
/// Ideally, this trait should extend ReadOnlyDataItemHandler<T>, but since Rust does not
/// support casting from `dyn DerivedTrait` to `dyn BaseTrait`, it's easier to simply
/// duplicate the read method is this trait.
/// See https://rust-lang.github.io/rfcs/3324-dyn-upcasting.html
#[async_trait::async_trait]
pub trait DataItemHandler<T>: Send + Sync
where
    T: MessageFull,
{
    /// Reads current data item value.
    async fn read(&self) -> DiagnosticsResult<T>;

    /// Reads current data item, converting it to the untyped ReadDataItemResponse.
    async fn read_untyped(&self) -> ReadDataItemResponse {
        to_read_response(self.read().await)
    }

    /// Writes new value to the data item.
    async fn write(&self, value: T) -> ResponseCode;

    /// Write new value to the data item, parsing it from bytes input.
    async fn write_untyped(&self, value_bytes: &[u8]) -> WriteDataItemResponse {
        let parsed_value = T::parse_from_bytes(value_bytes);

        match parsed_value {
            Ok(value) => {
                let result = self.write(value).await;
                WriteDataItemResponse { response_code: result.into(), ..Default::default() }
            }
            _ => WriteDataItemResponse {
                response_code: INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT.into(),
                ..Default::default()
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::data_item::{DataItemHandler, ReadOnlyDataItemHandler};
    use crate::DiagnosticsResult;
    use diagnostics_v1_api::data_item_service::{ReadDataItemResponse, WriteDataItemResponse};
    use diagnostics_v1_api::event::Event as TestProto;
    use diagnostics_v1_api::event::Status;
    use diagnostics_v1_api::response_code::ResponseCode;
    use protobuf::Message;

    struct TestReadOnlyHandler {
        read_result: DiagnosticsResult<TestProto>,
    }

    #[async_trait::async_trait]
    impl ReadOnlyDataItemHandler<TestProto> for TestReadOnlyHandler {
        async fn read(&self) -> DiagnosticsResult<TestProto> {
            self.read_result.clone()
        }
    }

    struct TestHandler {
        read_result: DiagnosticsResult<TestProto>,
        write_result: ResponseCode,
    }

    impl TestHandler {
        fn new_with_read_result(read_result: DiagnosticsResult<TestProto>) -> Self {
            Self { read_result, write_result: ResponseCode::POSITIVE_RESPONSE }
        }

        fn new_with_write_result(write_result: ResponseCode) -> Self {
            Self { read_result: Ok(TestProto { ..Default::default() }), write_result }
        }
    }

    #[async_trait::async_trait]
    impl DataItemHandler<TestProto> for TestHandler {
        async fn read(&self) -> DiagnosticsResult<TestProto> {
            self.read_result.clone()
        }

        async fn write(&self, _value: TestProto) -> ResponseCode {
            self.write_result
        }
    }

    #[tokio::test]
    async fn test_read_only_data_item_read_untyped_ok() {
        let message = TestProto { status: Status::PASS.into(), ..Default::default() };

        let handler = TestReadOnlyHandler { read_result: Ok(message.clone()) };

        assert_eq!(
            handler.read_untyped().await,
            ReadDataItemResponse { value: message.write_to_bytes().unwrap(), ..Default::default() }
        );
    }

    #[tokio::test]
    async fn test_read_only_data_item_read_untyped_err() {
        let handler = TestReadOnlyHandler { read_result: Err(ResponseCode::GENERAL_REJECT) };

        assert_eq!(
            handler.read_untyped().await,
            ReadDataItemResponse {
                response_code: ResponseCode::GENERAL_REJECT.into(),
                ..Default::default()
            }
        );
    }

    #[tokio::test]
    async fn test_data_item_read_untyped_ok() {
        let message = TestProto { status: Status::PASS.into(), ..Default::default() };

        let handler = TestHandler::new_with_read_result(DiagnosticsResult::Ok(message.clone()));

        assert_eq!(
            handler.read_untyped().await,
            ReadDataItemResponse { value: message.write_to_bytes().unwrap(), ..Default::default() }
        );
    }

    #[tokio::test]
    async fn test_data_item_read_untyped_err() {
        let handler =
            TestHandler::new_with_read_result(DiagnosticsResult::Err(ResponseCode::GENERAL_REJECT));

        assert_eq!(
            handler.read_untyped().await,
            ReadDataItemResponse {
                response_code: ResponseCode::GENERAL_REJECT.into(),
                ..Default::default()
            }
        );
    }

    #[tokio::test]
    async fn test_data_item_write_untyped_ok() {
        let message_bytes = TestProto { status: Status::PASS.into(), ..Default::default() }
            .write_to_bytes()
            .unwrap();

        let handler = TestHandler::new_with_write_result(ResponseCode::POSITIVE_RESPONSE);

        assert_eq!(
            handler.write_untyped(&message_bytes).await,
            WriteDataItemResponse { ..Default::default() }
        );
    }

    #[tokio::test]
    async fn test_data_item_write_untyped_err() {
        let message_bytes = TestProto { status: Status::PASS.into(), ..Default::default() }
            .write_to_bytes()
            .unwrap();

        let handler = TestHandler::new_with_write_result(ResponseCode::GENERAL_REJECT);

        assert_eq!(
            handler.write_untyped(&message_bytes).await,
            WriteDataItemResponse {
                response_code: ResponseCode::GENERAL_REJECT.into(),
                ..Default::default()
            }
        );
    }

    #[tokio::test]
    async fn test_data_item_write_untyped_format_err() {
        let invalid_message_bytes = vec![2, 1];

        let handler = TestHandler::new_with_write_result(ResponseCode::GENERAL_REJECT);

        assert_eq!(
            handler.write_untyped(&invalid_message_bytes).await,
            WriteDataItemResponse {
                response_code: ResponseCode::INCORRECT_MESSAGE_LENGTH_OR_INVALID_FORMAT.into(),
                ..Default::default()
            }
        );
    }
}
