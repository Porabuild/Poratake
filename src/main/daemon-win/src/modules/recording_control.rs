use crate::protocol::Request;
use crate::router::{Module, Reply, method_not_found};
use serde_json::json;

pub struct RecordingControlModule;

impl RecordingControlModule {
    pub fn new() -> Self {
        Self
    }
}

impl Module for RecordingControlModule {
    fn name(&self) -> &'static str {
        "recording-control"
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match request.method.as_str() {
            "listIOSDevices" => Reply::Now(Ok(Some(json!({ "devices": [] })))),
            method => method_not_found(method),
        }
    }
}
