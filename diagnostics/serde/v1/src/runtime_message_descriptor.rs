/*
 * Copyright (C) 2026 The Android Open Source Project
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

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
