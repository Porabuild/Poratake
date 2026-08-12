use crate::protocol::{send_response, Request, Response};
use serde_json::{json, Value};

pub type MethodResult = Result<Option<Value>, (String, String)>;

pub enum Reply {
    Now(MethodResult),
    Deferred,
}

impl From<MethodResult> for Reply {
    fn from(result: MethodResult) -> Self {
        Reply::Now(result)
    }
}

pub trait Module {
    fn name(&self) -> &'static str;

    fn handle(&mut self, request: &Request) -> Reply;
}

pub fn method_not_found(method: &str) -> Reply {
    Reply::Now(Err((
        "METHOD_NOT_FOUND".to_string(),
        format!("Unknown method: {method}"),
    )))
}

pub struct Router {
    modules: Vec<Box<dyn Module>>,
}

impl Router {
    pub fn new() -> Self {
        Router {
            modules: Vec::new(),
        }
    }

    pub fn register(&mut self, module: Box<dyn Module>) {
        self.modules.push(module);
    }

    pub fn route(&mut self, request: Request) {
        if request.module == "system" {
            self.handle_system(&request);
            return;
        }

        let Some(module) = self
            .modules
            .iter_mut()
            .find(|item| item.name() == request.module)
        else {
            send_response(Response::error(
                &request.id,
                "MODULE_NOT_FOUND",
                &format!("Module '{}' not found", request.module),
            ));
            return;
        };

        match module.handle(&request) {
            Reply::Now(Ok(result)) => send_response(Response::success(&request.id, result)),
            Reply::Now(Err((code, message))) => {
                send_response(Response::error(&request.id, &code, &message))
            }
            Reply::Deferred => {}
        }
    }

    fn handle_system(&self, request: &Request) {
        match request.method.as_str() {
            "ping" => {
                send_response(Response::success(
                    &request.id,
                    Some(json!({ "pong": true })),
                ));
            }
            "list-modules" => {
                let mut names: Vec<&str> = self.modules.iter().map(|item| item.name()).collect();
                names.sort_unstable();
                send_response(Response::success(
                    &request.id,
                    Some(json!({ "modules": names })),
                ));
            }
            "quit" => {
                send_response(Response::success(
                    &request.id,
                    Some(json!({ "status": "exiting" })),
                ));
                std::process::exit(0);
            }
            _ => {
                send_response(Response::error(
                    &request.id,
                    "METHOD_NOT_FOUND",
                    &format!("System method '{}' not found", request.method),
                ));
            }
        }
    }
}
