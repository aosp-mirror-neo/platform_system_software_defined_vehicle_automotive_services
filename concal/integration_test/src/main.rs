// Copyright 2024 Google LLC
//!  ConCal integration test.

#[cfg(test)]
mod tests {
    use sdv_samples_stdout_verifier::SamplesStdoutVerifier;

    const CONCAL_SERVER_BINARY: &str = "sdv_con_cal_service";
    const CONCAL_CLIENT_BINARY: &str = "sdv_con_cal_client";

    #[test]
    fn test_concal() {
        sdv_log::init_logger("concal").unwrap();

        let mut sample_logs_verifier = SamplesStdoutVerifier::new();

        sample_logs_verifier.register_sample(CONCAL_SERVER_BINARY);
        sample_logs_verifier
            .register_sample_with_expected_output(CONCAL_CLIENT_BINARY, "tests/client_stdout.txt");

        sample_logs_verifier.run_and_assert_stdout();
    }
}
