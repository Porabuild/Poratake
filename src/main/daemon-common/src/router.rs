use serde_json::{Value, json};

use crate::protocol::{Request, Response, send_response};

pub type MethodResult = Result<Option<Value>, (String, String)>;

pub enum Reply {
    Now(MethodResult),
    Deferred,
}

impl From<MethodResult> for Reply {
    fn from(result: MethodResult) -> Self {
        Self::Now(result)
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteControl {
    Continue,
    Exit,
}

pub struct Router {
    modules: Vec<Box<dyn Module>>,
    trace: Option<fn(&str, &str)>,
}

impl Router {
    pub fn new() -> Self {
        Self {
            modules: Vec::new(),
            trace: None,
        }
    }

    pub fn with_trace(trace: fn(&str, &str)) -> Self {
        Self {
            modules: Vec::new(),
            trace: Some(trace),
        }
    }

    pub fn register(&mut self, module: Box<dyn Module>) {
        self.modules.push(module);
    }

    pub fn validate_modules(&self, expected: &[&str]) -> Result<(), String> {
        let mut actual: Vec<_> = self.modules.iter().map(|module| module.name()).collect();
        let mut expected = expected.to_vec();
        actual.sort_unstable();
        expected.sort_unstable();
        if actual == expected {
            return Ok(());
        }
        Err(format!(
            "daemon modules do not match the contract: expected {expected:?}, got {actual:?}"
        ))
    }

    pub fn route(&mut self, request: Request) -> RouteControl {
        if let Some(trace) = self.trace {
            trace(&request.module, &request.method);
        }
        if request.module == "system" {
            return self.handle_system(&request);
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
            return RouteControl::Continue;
        };

        match module.handle(&request) {
            Reply::Now(Ok(result)) => send_response(Response::success(&request.id, result)),
            Reply::Now(Err((code, message))) => {
                send_response(Response::error(&request.id, &code, &message));
            }
            Reply::Deferred => {}
        }
        RouteControl::Continue
    }

    fn handle_system(&self, request: &Request) -> RouteControl {
        match request.method.as_str() {
            "ping" => {
                send_response(Response::success(
                    &request.id,
                    Some(json!({ "pong": true })),
                ));
                RouteControl::Continue
            }
            "list-modules" => {
                let mut names: Vec<&str> = self.modules.iter().map(|item| item.name()).collect();
                names.sort_unstable();
                send_response(Response::success(
                    &request.id,
                    Some(json!({ "modules": names })),
                ));
                RouteControl::Continue
            }
            "quit" => {
                send_response(Response::success(
                    &request.id,
                    Some(json!({ "status": "exiting" })),
                ));
                RouteControl::Exit
            }
            _ => {
                send_response(Response::error(
                    &request.id,
                    "METHOD_NOT_FOUND",
                    &format!("System method '{}' not found", request.method),
                ));
                RouteControl::Continue
            }
        }
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestModule(&'static str);

    impl Module for TestModule {
        fn name(&self) -> &'static str {
            self.0
        }

        fn handle(&mut self, _request: &Request) -> Reply {
            Reply::Now(Ok(None))
        }
    }

    #[test]
    fn module_validation_is_order_independent_and_exact() {
        let mut router = Router::new();
        router.register(Box::new(TestModule("second")));
        router.register(Box::new(TestModule("first")));

        assert_eq!(router.validate_modules(&["first", "second"]), Ok(()));
        assert!(router.validate_modules(&["first"]).is_err());
    }
}
