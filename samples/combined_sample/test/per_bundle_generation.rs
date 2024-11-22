// Copyright 2024 Google LLC

use googletest::assert_that;
use googletest::matchers::matches_pattern;
use googletest::prelude::eq;
use std::fs;

#[googletest::test]
fn test_bundle_files_generation() {
    assert_that!(fs::exists("vsidl-config.binpb"), matches_pattern!(Ok(eq(true))));
    assert_that!(fs::exists("diagnostics-config.binpb"), matches_pattern!(Ok(eq(true))));
}
