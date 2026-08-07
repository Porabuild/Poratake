use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::Write;

#[derive(Debug, Deserialize)]
pub struct Request {
    pub id: String,
    pub module: String,
    pub method: String,
    #[serde(default)]
    pub params: Option<HashMap<String, Value>>,
}

#[derive(Debug, Serialize)]
pub struct Response {
    pub id: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ResponseError>,
}

#[derive(Debug, Serialize)]
pub struct ResponseError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct Event {
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl Response {
    pub fn success(id: &str, result: Option<Value>) -> Self {
        Response {
            id: id.to_string(),
            success: true,
            result,
            error: None,
        }
    }

    pub fn error(id: &str, code: &str, message: &str) -> Self {
        Response {
            id: id.to_string(),
            success: false,
            result: None,
            error: Some(ResponseError {
                code: code.to_string(),
                message: message.to_string(),
            }),
        }
    }
}

fn write_line(json: String) {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    let _ = writeln!(lock, "{json}");
    let _ = lock.flush();
}

pub fn send_response(response: Response) {
    if let Ok(json) = serde_json::to_string(&response) {
        write_line(json);
    }
}

pub fn send_event(event: &str, data: Option<Value>) {
    let message = Event {
        event: event.to_string(),
        data,
    };
    if let Ok(json) = serde_json::to_string(&message) {
        write_line(json);
    }
}

pub fn respond_success(id: &str, result: Value) {
    send_response(Response::success(id, Some(result)));
}

pub fn respond_error(id: &str, code: &str, message: &str) {
    send_response(Response::error(id, code, message));
}

pub fn parse_request(line: &str) -> Option<Request> {
    serde_json::from_str(line).ok()
}

pub fn param_str<'a>(params: &'a Option<HashMap<String, Value>>, key: &str) -> Option<&'a str> {
    params.as_ref()?.get(key)?.as_str()
}

pub fn param_bool(params: &Option<HashMap<String, Value>>, key: &str) -> Option<bool> {
    params.as_ref()?.get(key)?.as_bool()
}

pub fn param_i64(params: &Option<HashMap<String, Value>>, key: &str) -> Option<i64> {
    let value = params.as_ref()?.get(key)?;
    value.as_i64().or_else(|| value.as_f64().map(|number| number as i64))
}

pub fn param_i32(params: &Option<HashMap<String, Value>>, key: &str) -> Option<i32> {
    let value = params.as_ref()?.get(key)?;
    if let Some(number) = value.as_i64() {
        return i32::try_from(number).ok();
    }
    value.as_f64().map(|number| number as i32)
}
