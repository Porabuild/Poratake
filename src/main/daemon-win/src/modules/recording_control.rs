use crate::protocol::Request;
use crate::router::{Module, Reply, method_not_found};
use poratake_daemon_common::contract::{RECORDING_CONTROL_MODULE, RecordingControlMethod};
use serde_json::json;

pub struct RecordingControlModule;

impl RecordingControlModule {
    pub fn new() -> Self {
        Self
    }
}

impl Module for RecordingControlModule {
    fn name(&self) -> &'static str {
        RECORDING_CONTROL_MODULE
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match RecordingControlMethod::parse(&request.method) {
            Some(RecordingControlMethod::ListIosDevices) => {
                Reply::Now(Ok(Some(json!({ "devices": [] }))))
            }
            None => method_not_found(&request.method),
        }
    }
}
