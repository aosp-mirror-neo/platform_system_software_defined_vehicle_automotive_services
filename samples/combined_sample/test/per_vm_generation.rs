// Copyright 2024 Google LLC

use googletest::assert_that;
use googletest::matchers::{eq, ok};
use std::fs;

#[googletest::test]
fn test_vm_files_generation() {
    assert_that!(fs::exists("vsidl-config.binpb"), ok(eq(&true)));
    assert_that!(fs::exists("someip-config.binpb"), ok(eq(&true)));
}
