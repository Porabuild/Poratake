use super::recorder_types::{RecorderError, RecordingConfig, RecordingResult};
use super::screen_capture::CaptureController;
use crate::protocol::{param_bool, respond_error, respond_success, send_event, Request};
use crate::router::{method_not_found, Module, Reply};
use serde_json::json;
use std::sync::mpsc::Receiver;

pub struct ScreenRecorderModule {
    recorder: CaptureController,
}

impl ScreenRecorderModule {
    pub fn new() -> Self {
        Self {
            recorder: CaptureController::new(),
        }
    }

    fn start(&self, request: &Request) -> Reply {
        let config = match RecordingConfig::from_request(request) {
            Ok(config) => config,
            Err(error) => return Reply::Now(Err((error.code.to_string(), error.message))),
        };
        let output_path = config.output_path.to_string_lossy().into_owned();
        let session = match self.recorder.start(config) {
            Ok(session) => session,
            Err(error) => return Reply::Now(Err((error.code.to_string(), error.message))),
        };
        let request_id = request.id.clone();

        std::thread::spawn(move || match session.started.recv() {
            Ok(Ok(status)) => {
                respond_success(
                    &request_id,
                    json!({
                        "success": true,
                        "state": status.state.as_str(),
                        "message": "Recording started",
                        "outputPath": output_path.clone(),
                    }),
                );
                send_event(
                    "screen-recorder:started",
                    Some(json!({ "outputPath": output_path })),
                );
                if let Some(error) = session.failure.recv() {
                    send_event(
                        "screen-recorder:error",
                        Some(json!({
                            "code": error.code,
                            "message": error.message,
                        })),
                    );
                }
            }
            Ok(Err(error)) => respond_error(&request_id, error.code, &error.message),
            Err(_) => respond_error(
                &request_id,
                "START_FAILED",
                "Recorder worker stopped before the first frame",
            ),
        });
        Reply::Deferred
    }

    fn pause(&self, request: &Request) -> Reply {
        match self.recorder.pause() {
            Ok(status) => {
                respond_success(
                    &request.id,
                    json!({
                        "success": true,
                        "state": status.state.as_str(),
                        "message": "Recording paused",
                        "duration": status.duration,
                    }),
                );
                send_event(
                    "screen-recorder:paused",
                    Some(json!({ "duration": status.duration })),
                );
            }
            Err(error) => respond_error(&request.id, error.code, &error.message),
        }
        Reply::Deferred
    }

    fn resume(&self, request: &Request) -> Reply {
        match self.recorder.resume() {
            Ok(status) => {
                respond_success(
                    &request.id,
                    json!({
                        "success": true,
                        "state": status.state.as_str(),
                        "message": "Recording resumed",
                        "duration": status.duration,
                    }),
                );
                send_event(
                    "screen-recorder:resumed",
                    Some(json!({ "duration": status.duration })),
                );
            }
            Err(error) => respond_error(&request.id, error.code, &error.message),
        }
        Reply::Deferred
    }

    fn stop(&self, request: &Request) -> Reply {
        let receiver = match self.recorder.stop() {
            Ok(receiver) => receiver,
            Err(error) => return Reply::Now(Err((error.code.to_string(), error.message))),
        };
        let request_id = request.id.clone();
        std::thread::spawn(move || finish_stop(&request_id, receiver));
        Reply::Deferred
    }
}

impl Module for ScreenRecorderModule {
    fn name(&self) -> &'static str {
        "screen-recorder"
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match request.method.as_str() {
            "start" => self.start(request),
            "pause" => self.pause(request),
            "resume" => self.resume(request),
            "stop" => self.stop(request),
            "status" => {
                let status = self.recorder.status();
                Reply::Now(Ok(Some(json!({
                    "state": status.state.as_str(),
                    "duration": status.duration,
                }))))
            }
            "setMicMuted" => {
                let muted = param_bool(&request.params, "muted").unwrap_or(false);
                self.recorder.set_mic_muted(muted);
                Reply::Now(Ok(Some(json!({
                    "success": true,
                    "muted": muted,
                }))))
            }
            method => method_not_found(method),
        }
    }
}

fn finish_stop(request_id: &str, receiver: Receiver<Result<RecordingResult, RecorderError>>) {
    match receiver.recv() {
        Ok(Ok(result)) => {
            let data = json!(&result);
            respond_success(
                request_id,
                json!({
                    "success": true,
                    "state": "idle",
                    "message": "Recording stopped",
                    "outputPath": data["outputPath"].clone(),
                    "cursorPath": data["cursorPath"].clone(),
                    "cameraPath": data["cameraPath"].clone(),
                    "keysPath": data["keysPath"].clone(),
                    "systemAudioPath": data["systemAudioPath"].clone(),
                    "micAudioPath": data["micAudioPath"].clone(),
                    "duration": result.duration,
                }),
            );
            send_event("screen-recorder:stopped", Some(data));
        }
        Ok(Err(error)) => respond_error(request_id, error.code, &error.message),
        Err(_) => respond_error(
            request_id,
            "STOP_FAILED",
            "Recorder worker stopped before finalizing the recording",
        ),
    }
}
