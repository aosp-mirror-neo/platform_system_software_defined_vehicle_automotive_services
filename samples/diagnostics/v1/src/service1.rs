// Copyright 2023 Google LLC

#![allow(missing_docs)]

use sdv_mw_rs_sdv_sample_diagnostics_diagnostics_provider1::DiagnosticsProvider1;

fn main() {
    let _service = DiagnosticsProvider1::new().unwrap();

    println!("Diagnostics provider service bundle started.");

    loop {
        std::thread::park();
    }
}
