#![allow(missing_docs)]
// Copyright 2023 Google LLC

use diagnostics_api::data_item::{ReadRequest, WriteRequest};
use sdvgenerated::data_item_interface::DataItem;
use sdvgenerated::diagnostics_manager_service::DiagnosticsManagerService;

fn main() {
    let data_item_id = "data_item_id";

    let service = DiagnosticsManagerService::new();
    println!("Diagnostics Manager service bundle started.");

    let read_request = ReadRequest { id: data_item_id.to_string(), ..Default::default() };

    let response1 = service.data_item_client.read(read_request);

    println!("Fetch {:?}: {:?}.", data_item_id, response1);

    let write_request =
        WriteRequest { id: data_item_id.to_string(), value: vec![2], ..Default::default() };

    let response2 = service.data_item_client.write(write_request);

    println!("Update {:?} to a new value {:?}. Response: {:?}.", data_item_id, vec![2], response2);

    let read_request = ReadRequest { id: data_item_id.to_string(), ..Default::default() };

    let response3 = service.data_item_client.read(read_request);

    println!("Fetch {:?} data item after update: {:?}.", data_item_id, response3);
}
