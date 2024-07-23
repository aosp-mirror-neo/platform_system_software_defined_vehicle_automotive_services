// Copyright 2024 Google LLC

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

pub(crate) use error_response;
