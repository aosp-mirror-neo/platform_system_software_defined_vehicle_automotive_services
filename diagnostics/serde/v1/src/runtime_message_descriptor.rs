// Copyright 2025 Google LLC

use protobuf::reflect::{FileDescriptor, MessageDescriptor};
use std::fs;
use vsidl_v1::{annotations, diagnostics};

pub fn get_file_descriptor(proto_text: &str) -> FileDescriptor {
    let temp_dir = tempfile::tempdir().unwrap();

    let tempfile = temp_dir.path().join("test_proto.proto");
    fs::write(&tempfile, proto_text).unwrap();

    let file_descriptor_proto = protobuf_parse::Parser::new()
        .pure()
        .include(&temp_dir)
        // current folder includes VSIDL syntax definitions that might be required
        // to properly parse test proto file.
        .include(".")
        .input(&tempfile)
        .file_descriptor_set()
        .unwrap()
        .file[0]
        .clone();

    FileDescriptor::new_dynamic(
        file_descriptor_proto.clone(),
        &[annotations::file_descriptor().clone(), diagnostics::file_descriptor().clone()],
    )
    .unwrap()
}

pub fn get_message_descriptor(proto_text: &str, message_name: &str) -> MessageDescriptor {
    get_file_descriptor(proto_text).message_by_full_name(message_name).unwrap()
}
