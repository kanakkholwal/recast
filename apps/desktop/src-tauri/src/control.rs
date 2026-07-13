//! Local control channel between the `recast` CLI and a running app.
//!
//! The GUI hosts a small server on an OS local socket (named pipe on Windows,
//! Unix socket on macOS/Linux via `interprocess`). The CLI connects, sends one
//! JSON request line, and reads one JSON response line. This is how live
//! commands (`status`, `rec ...`) reach an already-running instance, since the
//! single-instance argv path is one-way and cannot answer a query.
//!
//! Auth: the server writes a random token to a 0600 file in the temp dir; the
//! CLI reads it and includes it in every request. Same-user gating comes from
//! the socket/pipe ACL; the token is defense in depth. Phase 2 is synchronous
//! request/response only; the event stream (`watch`) lands later.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use interprocess::local_socket::prelude::*;
use interprocess::local_socket::{GenericNamespaced, ListenerOptions, Stream};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Namespaced socket name. Maps to `\\.\pipe\<name>` on Windows and an abstract
/// or temp-dir socket on Unix. One app per user, so a fixed name is fine.
const SOCKET_NAME: &str = "com.kanakkholwal.recast.cli.sock";

/// Path to the auth token file. Both server and client compute it without an
/// `AppHandle`, so the headless CLI can find it too.
fn token_path() -> PathBuf {
    std::env::temp_dir().join("com.kanakkholwal.recast.cli.token")
}

#[derive(Deserialize)]
struct Request {
    token: String,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Serialize, Deserialize)]
struct Response {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

// ---------------------------------------------------------------------------
// Server (app side)
// ---------------------------------------------------------------------------

/// Start the control server on a background thread. Non-fatal on failure: the
/// GUI still works, the CLI just can't reach this instance.
pub fn spawn_server(app: tauri::AppHandle) {
    let _ = std::thread::Builder::new()
        .name("recast-cli-control".into())
        .spawn(move || {
            if let Err(e) = run_server(&app) {
                log::warn!("cli control server stopped: {e}");
            }
        });
}

fn run_server(app: &tauri::AppHandle) -> Result<(), String> {
    let token = write_token()?;
    let name = SOCKET_NAME
        .to_ns_name::<GenericNamespaced>()
        .map_err(|e| e.to_string())?;
    let listener = ListenerOptions::new()
        .name(name)
        .create_sync()
        .map_err(|e| format!("bind {SOCKET_NAME}: {e}"))?;
    log::info!("cli control server listening on {SOCKET_NAME}");

    // One thread per connection so a long-lived `watch` never blocks other
    // commands (`status`, `rec ...`) from being accepted and answered.
    for incoming in listener.incoming() {
        match incoming {
            Ok(mut stream) => {
                let app = app.clone();
                let token = token.clone();
                std::thread::spawn(move || {
                    if let Err(e) = handle_conn(&app, &mut stream, &token) {
                        log::debug!("cli control connection error: {e}");
                    }
                });
            }
            Err(e) => log::debug!("cli control accept error: {e}"),
        }
    }
    Ok(())
}

fn err_response(message: String) -> Response {
    Response {
        ok: false,
        result: None,
        error: Some(message),
    }
}

fn write_frame(stream: &mut Stream, frame: &str) -> Result<(), String> {
    stream
        .write_all(frame.as_bytes())
        .map_err(|e| e.to_string())?;
    stream.write_all(b"\n").map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())?;
    Ok(())
}

fn write_response(stream: &mut Stream, response: &Response) -> Result<(), String> {
    let json = serde_json::to_string(response).map_err(|e| e.to_string())?;
    write_frame(stream, &json)
}

fn handle_conn(app: &tauri::AppHandle, stream: &mut Stream, token: &str) -> Result<(), String> {
    let mut line = String::new();
    {
        let mut reader = BufReader::new(&mut *stream);
        reader.read_line(&mut line).map_err(|e| e.to_string())?;
    }

    let req = match serde_json::from_str::<Request>(&line) {
        Ok(r) => r,
        Err(e) => return write_response(stream, &err_response(format!("bad request: {e}"))),
    };
    if req.token != token {
        return write_response(stream, &err_response("unauthorized".into()));
    }
    // `watch` takes over the connection and streams frames until the client
    // hangs up, rather than returning a single response.
    if req.method == "watch" {
        return handle_watch(app, stream, &req.params);
    }

    let response = match dispatch(app, &req.method, req.params) {
        Ok(result) => Response {
            ok: true,
            result: Some(result),
            error: None,
        },
        Err(error) => err_response(error),
    };
    write_response(stream, &response)
}

/// Resolve the requested event-group names (`rec`, `selection`) to concrete
/// event names. Empty/absent selects everything.
fn watch_event_names(params: &Value) -> Vec<String> {
    const REC: &[&str] = &["recording:started", "recording:stopped"];
    const SELECTION: &[&str] = &["capture-intent:changed"];
    const PROFILES: &[&str] = &["recording-profiles:changed"];
    let mut out: Vec<String> = Vec::new();
    match params.get("events").and_then(Value::as_array) {
        Some(groups) if !groups.is_empty() => {
            for group in groups {
                match group.as_str() {
                    Some("rec") => out.extend(REC.iter().map(|s| s.to_string())),
                    Some("selection") => out.extend(SELECTION.iter().map(|s| s.to_string())),
                    Some("profiles") => out.extend(PROFILES.iter().map(|s| s.to_string())),
                    _ => {}
                }
            }
        }
        _ => {
            out.extend(REC.iter().map(|s| s.to_string()));
            out.extend(SELECTION.iter().map(|s| s.to_string()));
            out.extend(PROFILES.iter().map(|s| s.to_string()));
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Subscribe to the requested events and forward each as a `{event, data}` frame
/// until the client hangs up. A 15s heartbeat (`{"event":"ping"}`) detects a
/// disconnect during an idle stretch so the listeners get unregistered.
fn handle_watch(app: &tauri::AppHandle, stream: &mut Stream, params: &Value) -> Result<(), String> {
    use std::sync::mpsc::{channel, RecvTimeoutError};
    use std::time::Duration;
    use tauri::Listener;

    let names = watch_event_names(params);
    let (tx, rx) = channel::<String>();
    let mut ids = Vec::new();
    for name in &names {
        let tx = tx.clone();
        let event_name = name.clone();
        let id = app.listen(name.clone(), move |event| {
            let data: Value = serde_json::from_str(event.payload()).unwrap_or(Value::Null);
            let _ = tx.send(json!({ "event": event_name, "data": data }).to_string());
        });
        ids.push(id);
    }
    drop(tx); // only the listener-held senders keep the channel open

    let ready = json!({ "event": "watch.ready", "data": { "events": names } }).to_string();
    let outcome = write_frame(stream, &ready).and_then(|()| loop {
        match rx.recv_timeout(Duration::from_secs(15)) {
            Ok(frame) => write_frame(stream, &frame)?,
            Err(RecvTimeoutError::Timeout) => write_frame(stream, "{\"event\":\"ping\"}")?,
            Err(RecvTimeoutError::Disconnected) => break Ok(()),
        }
    });

    for id in ids {
        app.unlisten(id);
    }
    // A write error just means the client hung up; not a server error.
    let _ = outcome;
    Ok(())
}

/// Params for `rec.start`, mirroring `start_recording`'s arguments.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartParams {
    target_type: String,
    target_id: u32,
    #[serde(default)]
    region: Option<crate::recording::RegionRect>,
    #[serde(default)]
    options: Option<crate::recording::RecordingOptions>,
}

/// Merge a partial patch (only the keys the CLI verb supplied) into the intent.
/// Key presence is authoritative, so `microphoneDeviceId: null` clears the id
/// while an absent key leaves it untouched.
fn apply_patch(intent: &mut crate::commands::types::CaptureIntent, params: &Value) {
    let Some(map) = params.as_object() else {
        return;
    };
    if let Some(v) = map.get("targetType") {
        intent.target_type = v.as_str().map(str::to_string);
    }
    if let Some(n) = map.get("targetId").and_then(Value::as_u64) {
        intent.target_id = n as u32;
    }
    if map.contains_key("region") {
        intent.region = serde_json::from_value(map["region"].clone()).ok();
    }
    if let Some(b) = map.get("systemAudio").and_then(Value::as_bool) {
        intent.options.system_audio = b;
    }
    if let Some(b) = map.get("microphone").and_then(Value::as_bool) {
        intent.options.microphone = b;
    }
    if map.contains_key("microphoneDeviceId") {
        intent.options.microphone_device_id =
            map["microphoneDeviceId"].as_str().map(str::to_string);
    }
    if let Some(b) = map.get("camera").and_then(Value::as_bool) {
        intent.options.camera = b;
    }
    if map.contains_key("cameraDeviceId") {
        intent.options.camera_device_id = map["cameraDeviceId"].as_str().map(str::to_string);
    }
    if map.contains_key("fps") {
        intent.options.fps = map["fps"].as_u64().map(|n| n as u32);
    }
    if map.contains_key("quality") {
        intent.options.quality = map["quality"].as_str().map(str::to_string);
    }
    if map.contains_key("countdown") {
        intent.countdown = map["countdown"].as_u64().map(|n| n as u32);
    }
}

/// Route a method to the existing command functions. Reuses the exact command
/// logic (power management, project write) by handing them a `State` obtained
/// from the `AppHandle`, so there is no duplicated recording logic here.
fn dispatch(app: &tauri::AppHandle, method: &str, params: Value) -> Result<Value, String> {
    use tauri::Manager;
    let state = app.state::<crate::commands::types::AppState>();
    let manager = &state.recording_manager;

    match method {
        "status" => Ok(json!({
            "recording": manager.is_recording(),
            "paused": manager.is_paused(),
            "version": app.package_info().version.to_string(),
        })),
        "rec.status" => Ok(json!({
            "recording": manager.is_recording(),
            "paused": manager.is_paused(),
        })),
        "intent.get" => {
            serde_json::to_value(crate::commands::get_intent(app)).map_err(|e| e.to_string())
        }
        "intent.reset" => {
            let next = crate::commands::update_intent(app, |i| *i = Default::default());
            serde_json::to_value(next).map_err(|e| e.to_string())
        }
        "intent.patch" => {
            let next = crate::commands::update_intent(app, |i| apply_patch(i, &params));
            serde_json::to_value(next).map_err(|e| e.to_string())
        }
        "profile.list" => {
            serde_json::to_value(crate::commands::profiles_snapshot(app)).map_err(|e| e.to_string())
        }
        "profile.use" => {
            let id = params
                .get("id")
                .and_then(Value::as_str)
                .ok_or("profile.use requires an id")?;
            let intent = crate::commands::use_profile_by_id(app, id)?;
            serde_json::to_value(intent).map_err(|e| e.to_string())
        }
        "app.screenshot" => {
            use crate::commands::screenshot::{ShotOptions, DEFAULT_MAX_EDGE};
            let label = params.get("window").and_then(Value::as_str);
            let opts = ShotOptions {
                out: params.get("out").and_then(Value::as_str).map(PathBuf::from),
                // Absent => default cap; the CLI sends 0 for a full-res request.
                max_edge: params
                    .get("maxEdge")
                    .and_then(Value::as_u64)
                    .map(|n| n as u32)
                    .unwrap_or(DEFAULT_MAX_EDGE),
                base64: params
                    .get("base64")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            };
            // xcap capture can stall; safe here because each connection runs on
            // its own thread, never the GTK/main thread.
            let shot = crate::commands::screenshot::capture_app_window(app, label, &opts)?;
            serde_json::to_value(shot).map_err(|e| e.to_string())
        }
        "rec.start" => {
            // Read the auto-stop before `params` is consumed below.
            let timeout_ms = params.get("timeoutMs").and_then(Value::as_u64);
            // Explicit target flags are a one-off; without them we record the
            // stored capture intent (set via `recast select`/`set`).
            let explicit = params.get("targetType").and_then(|v| v.as_str()).is_some();
            let (target_type, target_id, region, options) = if explicit {
                let p: StartParams = serde_json::from_value(params).map_err(|e| e.to_string())?;
                (p.target_type, p.target_id, p.region, p.options)
            } else {
                let intent = crate::commands::get_intent(app);
                match intent.target_type {
                    Some(tt) => (tt, intent.target_id, intent.region, Some(intent.options)),
                    None => {
                        return Err("no source selected. Pass --screen/--window/--region, or run `recast select ...` first.".into())
                    }
                }
            };
            let result = tauri::async_runtime::block_on(crate::commands::start_recording(
                app.clone(),
                target_type,
                target_id,
                region,
                options,
                state,
            ))
            .map_err(|e| e.to_string())?;
            // Backend-owned auto-stop: survives the CLI process exiting. Only
            // stops if still recording (a manual stop first is a no-op).
            if let Some(ms) = timeout_ms {
                schedule_auto_stop(app.clone(), ms);
            }
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        "rec.stop" => {
            let path =
                tauri::async_runtime::block_on(crate::commands::stop_recording(app.clone(), state))
                    .map_err(|e| e.to_string())?;
            Ok(json!({ "projectPath": path }))
        }
        "rec.pause" => {
            crate::commands::pause_recording(state).map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        "rec.resume" => {
            crate::commands::resume_recording(state).map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        // On-device OCR of a video into a structured text timeline. No previews:
        // an agent consumes the structured elements, not base64 images.
        "screen.read" => {
            let path = params
                .get("path")
                .and_then(Value::as_str)
                .ok_or("screen.read requires a path")?;
            // block_on is safe here: each connection runs on its own thread. No
            // previews (an agent reads the structured elements, not base64 images)
            // and no range filter: the CLI is handed a bare file with no edit
            // context, so the whole thing is the clip.
            let timeline = tauri::async_runtime::block_on(crate::ocr::run(
                app,
                path,
                false,
                Vec::new(),
                |_| {},
            ))?;
            serde_json::to_value(timeline).map_err(|e| e.to_string())
        }
        other => Err(format!("unknown method: {other}")),
    }
}

/// Stop the recording after `ms` on Tauri's runtime. Runs independently of the
/// CLI connection, so `recast rec start --timeout 30s` finalizes even after the
/// CLI process exits. A no-op if a manual stop already ended the session.
fn schedule_auto_stop(app: tauri::AppHandle, ms: u64) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
        use tauri::Manager;
        let state = app.state::<crate::commands::types::AppState>();
        if state.recording_manager.is_recording() {
            if let Err(e) = crate::commands::stop_recording(app.clone(), state).await {
                log::warn!("auto-stop failed: {e}");
            }
        }
    });
}

fn write_token() -> Result<String, String> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let token: String = (0..32)
        .map(|_| format!("{:x}", rng.gen_range(0..16)))
        .collect();
    let path = token_path();
    std::fs::write(&path, &token).map_err(|e| format!("write token: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(token)
}

// ---------------------------------------------------------------------------
// Client (CLI side)
// ---------------------------------------------------------------------------

/// Send one request to the running app and return its result value. Auto-launches
/// the app (unless `auto_launch` is false) and waits up to `timeout_ms` for the
/// server + token to come up.
pub fn send(
    method: &str,
    params: Value,
    auto_launch: bool,
    timeout_ms: u64,
) -> Result<Value, String> {
    let mut stream = match connect() {
        Ok(s) => s,
        Err(_) if auto_launch => {
            launch_app()?;
            connect_with_retry(timeout_ms)?
        }
        Err(e) => {
            return Err(format!(
                "Recast is not running ({e}). Launch it, or drop --no-launch to auto-start."
            ))
        }
    };

    let token = read_token_with_retry(timeout_ms)?;
    let mut req = serde_json::to_string(&json!({
        "token": token,
        "method": method,
        "params": params,
    }))
    .map_err(|e| e.to_string())?;
    req.push('\n');
    stream
        .write_all(req.as_bytes())
        .map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())?;

    let mut line = String::new();
    BufReader::new(&mut stream)
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    let response: Response = serde_json::from_str(&line).map_err(|e| e.to_string())?;
    if response.ok {
        Ok(response.result.unwrap_or(Value::Null))
    } else {
        Err(response.error.unwrap_or_else(|| "unknown error".into()))
    }
}

/// Open a `watch` stream and invoke `on_frame` for each event frame until the
/// server closes the connection (app exit / interrupt).
pub fn watch(
    params: Value,
    auto_launch: bool,
    timeout_ms: u64,
    mut on_frame: impl FnMut(&Value),
) -> Result<(), String> {
    let mut stream = match connect() {
        Ok(s) => s,
        Err(_) if auto_launch => {
            launch_app()?;
            connect_with_retry(timeout_ms)?
        }
        Err(e) => {
            return Err(format!(
                "Recast is not running ({e}). Launch it, or drop --no-launch to auto-start."
            ))
        }
    };
    let token = read_token_with_retry(timeout_ms)?;
    let mut req = serde_json::to_string(&json!({
        "token": token,
        "method": "watch",
        "params": params,
    }))
    .map_err(|e| e.to_string())?;
    req.push('\n');
    stream
        .write_all(req.as_bytes())
        .map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())?;

    let reader = BufReader::new(&mut stream);
    for line in reader.lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let frame: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        // Heartbeats are internal; don't surface them.
        if frame.get("event").and_then(Value::as_str) == Some("ping") {
            continue;
        }
        on_frame(&frame);
    }
    Ok(())
}

fn connect() -> Result<Stream, String> {
    let name = SOCKET_NAME
        .to_ns_name::<GenericNamespaced>()
        .map_err(|e| e.to_string())?;
    Stream::connect(name).map_err(|e| e.to_string())
}

fn connect_with_retry(timeout_ms: u64) -> Result<Stream, String> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    loop {
        match connect() {
            Ok(s) => return Ok(s),
            Err(e) if std::time::Instant::now() >= deadline => {
                return Err(format!("timed out waiting for Recast to start: {e}"))
            }
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(120)),
        }
    }
}

fn read_token_with_retry(timeout_ms: u64) -> Result<String, String> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    loop {
        match std::fs::read_to_string(token_path()) {
            Ok(t) if !t.trim().is_empty() => return Ok(t.trim().to_string()),
            _ if std::time::Instant::now() >= deadline => {
                return Err("timed out waiting for the control token".into())
            }
            _ => std::thread::sleep(std::time::Duration::from_millis(120)),
        }
    }
}

fn launch_app() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    std::process::Command::new(exe)
        .spawn()
        .map_err(|e| format!("failed to launch Recast: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::types::CaptureIntent;
    use serde_json::json;

    #[test]
    fn patch_merges_fields_without_clobbering() {
        let mut intent = CaptureIntent::default();
        assert!(intent.options.system_audio); // default on
        apply_patch(&mut intent, &json!({"targetType":"display","targetId":3}));
        apply_patch(
            &mut intent,
            &json!({"microphone":true,"microphoneDeviceId":"mic-1"}),
        );
        apply_patch(&mut intent, &json!({"systemAudio":false}));
        apply_patch(&mut intent, &json!({"fps":60}));
        assert_eq!(intent.target_type.as_deref(), Some("display"));
        assert_eq!(intent.target_id, 3);
        assert!(intent.options.microphone);
        assert_eq!(
            intent.options.microphone_device_id.as_deref(),
            Some("mic-1")
        );
        assert!(!intent.options.system_audio);
        assert_eq!(intent.options.fps, Some(60));
    }

    #[test]
    fn patch_null_clears_but_absent_preserves() {
        let mut intent = CaptureIntent::default();
        apply_patch(
            &mut intent,
            &json!({"microphone":true,"microphoneDeviceId":"mic-1"}),
        );
        assert_eq!(
            intent.options.microphone_device_id.as_deref(),
            Some("mic-1")
        );
        // A patch that does not mention the id leaves it alone.
        apply_patch(&mut intent, &json!({"fps":30}));
        assert_eq!(
            intent.options.microphone_device_id.as_deref(),
            Some("mic-1")
        );
        // An explicit null clears it.
        apply_patch(&mut intent, &json!({"microphoneDeviceId":null}));
        assert_eq!(intent.options.microphone_device_id, None);
    }

    #[test]
    fn intent_serializes_camelcase_and_omits_none() {
        let v = serde_json::to_value(CaptureIntent::default()).unwrap();
        assert!(v.get("targetType").is_none());
        assert!(v.get("region").is_none());
        assert!(v.get("countdown").is_none());
        assert!(v.get("activeProfileId").is_none());
        assert_eq!(v["targetId"], json!(0));
        assert_eq!(v["options"]["systemAudio"], json!(true));
        assert!(v["options"].get("fps").is_none());
        assert!(v["options"].get("microphoneDeviceId").is_none());
    }

    #[test]
    fn watch_names_default_all_and_by_group() {
        let all = watch_event_names(&Value::Null);
        assert!(all.contains(&"recording:started".to_string()));
        assert!(all.contains(&"recording:stopped".to_string()));
        assert!(all.contains(&"capture-intent:changed".to_string()));
        assert!(all.contains(&"recording-profiles:changed".to_string()));

        assert_eq!(
            watch_event_names(&json!({"events":["rec"]})),
            vec![
                "recording:started".to_string(),
                "recording:stopped".to_string()
            ]
        );
        assert_eq!(
            watch_event_names(&json!({"events":["selection"]})),
            vec!["capture-intent:changed".to_string()]
        );
        assert_eq!(
            watch_event_names(&json!({"events":["profiles"]})),
            vec!["recording-profiles:changed".to_string()]
        );
    }
}
