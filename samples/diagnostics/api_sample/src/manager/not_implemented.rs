#[macro_export]
/// Responds with a generic diagnostics error status
macro_rules! not_implemented {
    () => {{
        Result::Err(binder::Status::new_service_specific_error(
            ResponseCode::GENERAL_REJECT.0.into(),
            None,
        ))
    }};
}
