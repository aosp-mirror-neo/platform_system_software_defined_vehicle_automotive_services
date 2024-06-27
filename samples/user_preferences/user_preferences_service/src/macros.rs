// Copyright 2024 Google LLC

macro_rules! acquire_lock_or_return_err {
    ($mutex:expr,$return_type:ident) => {{
        match $mutex.lock() {
            Ok(x) => x,
            Err(e) => {
                return Ok($return_type {
                    error: MessageField::some(Error {
                        error_code: EnumOrUnknown::from(ResponseCode::RUNTIME_ERROR),
                        message: format!("Unable to obtain mutex: {:?}", e),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
            }
        }
    }};
}

macro_rules! error_response {
    ($return_type:ident, $response_code:expr, $error_message:literal, $($error_message_format_arg: tt)*) => {{
        Ok($return_type {
            error: MessageField::some(Error {
                error_code: EnumOrUnknown::from($response_code),
                message: format!($error_message, $($error_message_format_arg)*),
                ..Default::default()
            }),
            ..Default::default()
        })
    }};
}

pub(crate) use acquire_lock_or_return_err;
pub(crate) use error_response;
