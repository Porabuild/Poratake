use poratake_daemon_common::contract::{WINDOW_SELECTOR_MODULE, WindowSelectorMethod};
use poratake_daemon_common::protocol::Request;
use poratake_daemon_common::router::{Module, Reply, method_not_found};
use serde_json::json;

use crate::Backend;

pub struct WindowSelectorModule {
    backend: Backend,
}

impl WindowSelectorModule {
    pub fn new(backend: Backend) -> Self {
        Self { backend }
    }
}

impl Module for WindowSelectorModule {
    fn name(&self) -> &'static str {
        WINDOW_SELECTOR_MODULE
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match WindowSelectorMethod::parse(&request.method) {
            Some(WindowSelectorMethod::List) => match crate::capture::list_windows(self.backend) {
                Ok(windows) => Reply::Now(Ok(Some(json!({ "windows": windows })))),
                Err(error) => Reply::Now(Err(("WINDOW_LIST_FAILED".into(), error.to_string()))),
            },
            None => method_not_found(&request.method),
        }
    }
}
