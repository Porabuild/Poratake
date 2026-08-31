use super::recorder_types::{RecorderError, RecordingConfig, RecordingResult};
use super::recording_audio::AudioDevice;
use super::screen_capture::CaptureController;
use crate::protocol::{
    Request, params as parse_params, respond_error, respond_success, send_event,
};
use crate::router::{Module, Reply, method_not_found};
use poratake_daemon_common::contract::{
    SCREEN_RECORDER_MODULE, ScreenRecorderMethod, ScreenRecorderMicrophoneRequest,
    ScreenRecorderToggleRequest,
};
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

    fn set_microphone(&self, request: &Request) -> Reply {
        let params: ScreenRecorderMicrophoneRequest = match parse_params(request) {
            Ok(params) => params,
            Err(error) => return Reply::Now(Err(error)),
        };
        let device = params.enabled.then_some(AudioDevice {
            id: params.device_id,
            name: params.device_name,
        });
        device_reply(self.recorder.set_microphone(device), params.enabled)
    }

    fn set_system_audio(&self, request: &Request) -> Reply {
        let params: ScreenRecorderToggleRequest = match parse_params(request) {
            Ok(params) => params,
            Err(error) => return Reply::Now(Err(error)),
        };
        device_reply(
            self.recorder.set_system_audio(params.enabled),
            params.enabled,
        )
    }

    fn set_camera(&self, request: &Request) -> Reply {
        let params: ScreenRecorderToggleRequest = match parse_params(request) {
            Ok(params) => params,
            Err(error) => return Reply::Now(Err(error)),
        };
        device_reply(self.recorder.set_camera(params.enabled), params.enabled)
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
        SCREEN_RECORDER_MODULE
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match ScreenRecorderMethod::parse(&request.method) {
            Some(ScreenRecorderMethod::Start) => self.start(request),
            Some(ScreenRecorderMethod::Pause) => self.pause(request),
            Some(ScreenRecorderMethod::Resume) => self.resume(request),
            Some(ScreenRecorderMethod::Stop) => self.stop(request),
            Some(ScreenRecorderMethod::Status) => {
                let status = self.recorder.status();
                Reply::Now(Ok(Some(json!({
                    "state": status.state.as_str(),
                    "duration": status.duration,
                }))))
            }
            Some(ScreenRecorderMethod::SetMicrophone) => self.set_microphone(request),
            Some(ScreenRecorderMethod::SetSystemAudio) => self.set_system_audio(request),
            Some(ScreenRecorderMethod::SetCamera) => self.set_camera(request),
            None => method_not_found(&request.method),
        }
    }
}

fn device_reply(result: Result<(), RecorderError>, enabled: bool) -> Reply {
    match result {
        Ok(()) => Reply::Now(Ok(Some(json!({
            "success": true,
            "enabled": enabled,
        })))),
        Err(error) => Reply::Now(Err((error.code.to_string(), error.message))),
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
