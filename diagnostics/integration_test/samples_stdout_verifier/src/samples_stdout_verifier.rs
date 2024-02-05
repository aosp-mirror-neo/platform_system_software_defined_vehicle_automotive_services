// Copyright 2024 Google LLC
//! Utility to run a setup of several samples, verifying their stdout.
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::option::Option::{None, Some};
use std::process::Command;
use std::thread;
use std::vec::Vec;

// Information about registered sample.
//
// Samples can be registered with or without expected stdout.
struct SampleInfo {
    binary_path: &'static str,
    expected_stdout_file: Option<&'static str>,
}

/// Utility to run a setup of several samples, verifying their stdout.
pub struct SamplesStdoutVerifier {
    samples: Vec<SampleInfo>,
}

impl SamplesStdoutVerifier {
    /// Creates new SamplesStdoutVerifier with default timeout.
    pub fn new() -> Self {
        Self { samples: Vec::new() }
    }

    /// Registers sample to be run in `run_and_assert_stdout`.
    pub fn register_sample(&mut self, binary_path: &'static str) {
        self.samples.push(SampleInfo { binary_path, expected_stdout_file: None })
    }

    /// Registers sample to be run in `run_and_assert_stdout` together with expected
    /// stdout.
    ///
    /// Sample registered with this method is expected to run to completion on its own.
    pub fn register_sample_with_expected_output(
        &mut self,
        binary_path: &'static str,
        expected_stdout_file: &'static str,
    ) {
        self.samples
            .push(SampleInfo { binary_path, expected_stdout_file: Some(expected_stdout_file) })
    }

    /// Runs registered samples in registration order, verifying their stdout outputs.
    pub fn run_and_assert_stdout(self) {
        let mut children = Vec::new();
        let mut children_join_handles = Vec::new();

        self.samples.iter().for_each(|sample_info| {
            let sample_name = sample_info.binary_path;

            match sample_info.expected_stdout_file {
                Some(expected_stdout_file) => {
                    children_join_handles.push(thread::spawn(move || {
                        let child_output = Command::new(sample_name)
                            .output()
                            .unwrap_or_else(|_| panic!("Failed to capture {} output", sample_name));

                        let child_string_output = String::from_utf8_lossy(&child_output.stdout);

                        Self::assert_eq_to_file(&child_string_output, expected_stdout_file)
                    }));
                }
                None => {
                    let child = Command::new(sample_name)
                        .spawn()
                        .unwrap_or_else(|_| panic!("Failed to spawn {}", sample_name));
                    children.push(child);
                }
            }
        });

        children_join_handles.into_iter().for_each(|child_join_handle| {
            child_join_handle.join().expect("Failed to wait for child to finish")
        });

        children
            .into_iter()
            .for_each(|mut child| child.kill().expect("Failed to kill child process"));
    }

    fn assert_eq_to_file(output: &str, expected_output_file_path: &str) {
        let expected_output = File::open(expected_output_file_path).unwrap();

        let output_lines = output.lines();
        let mut expected_output_lines = BufReader::new(expected_output).lines();

        output_lines.for_each(|actual_line| {
            let expected_line = expected_output_lines.next().unwrap().unwrap();
            assert_eq!(actual_line, expected_line);
        })
    }
}

impl Default for SamplesStdoutVerifier {
    fn default() -> Self {
        Self::new()
    }
}
