// Copyright 2024 Google LLC
//!  Diagnostics integration test.

#[cfg(test)]
mod tests {
    use sdv_samples_stdout_verifier::SamplesStdoutVerifier;

    const DIAGNOSTICS_MANAGER_BINARY: &str = "diag_manager_sample";
    const DIAGNOSTICS_SERVICE1_BINARY: &str = "diag_provider1_sample";
    const DIAGNOSTICS_SERVICE2_BINARY: &str = "diag_provider2_sample";

    #[test]
    fn test_diagnostics() {
        sdv_log::init_logger("diagnostics").unwrap();

        let mut sample_logs_verifier = SamplesStdoutVerifier::new();

        sample_logs_verifier.register_sample(DIAGNOSTICS_SERVICE1_BINARY);
        sample_logs_verifier.register_sample(DIAGNOSTICS_SERVICE2_BINARY);
        sample_logs_verifier.register_sample_with_expected_output(
            DIAGNOSTICS_MANAGER_BINARY,
            "tests/manager_stdout.txt",
        );

        sample_logs_verifier.run_and_assert_stdout_ignore_order();
    }
}
