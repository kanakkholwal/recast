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
use tauri::Emitter;

use crate::render::graph::RenderState;

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
    const EXPORT: &[&str] = &["export-state", "export-jobs-changed"];
    const EDITOR: &[&str] = &["editor-session:changed", "editor-state:changed"];
    let mut out: Vec<String> = Vec::new();
    match params.get("events").and_then(Value::as_array) {
        Some(groups) if !groups.is_empty() => {
            for group in groups {
                match group.as_str() {
                    Some("rec") => out.extend(REC.iter().map(|s| s.to_string())),
                    Some("selection") => out.extend(SELECTION.iter().map(|s| s.to_string())),
                    Some("profiles") => out.extend(PROFILES.iter().map(|s| s.to_string())),
                    Some("export") => out.extend(EXPORT.iter().map(|s| s.to_string())),
                    Some("editor") => out.extend(EDITOR.iter().map(|s| s.to_string())),
                    _ => {}
                }
            }
        }
        _ => {
            out.extend(REC.iter().map(|s| s.to_string()));
            out.extend(SELECTION.iter().map(|s| s.to_string()));
            out.extend(PROFILES.iter().map(|s| s.to_string()));
            out.extend(EXPORT.iter().map(|s| s.to_string()));
            out.extend(EDITOR.iter().map(|s| s.to_string()));
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
                |_| {}, // a one-shot CLI call has nowhere to render progress
            ))?;
            serde_json::to_value(timeline).map_err(|e| e.to_string())
        }
        // Phase A (read-only): editor + export introspection for CLI agents.
        // Each path loads the project on demand through the same
        // `load_editor_document`/`list_export_jobs` paths the GUI uses; nothing
        // here mutates state, so the future `EditorSession` write-lock doesn't
        // need to be touched for v1.
        "editor.open" => {
            let path = params
                .get("path")
                .and_then(Value::as_str)
                .ok_or("editor.open requires a path")?;
            let doc = tauri::async_runtime::block_on(crate::commands::load_editor_document(
                path.to_string(),
            ))
            .map_err(|e| e.to_string())?;
            serde_json::to_value(doc).map_err(|e| e.to_string())
        }
        "editor.show" => {
            let path = params
                .get("path")
                .and_then(Value::as_str)
                .ok_or("editor.show requires a path")?;
            let doc = tauri::async_runtime::block_on(crate::commands::load_editor_document(
                path.to_string(),
            ))
            .map_err(|e| e.to_string())?;
            serde_json::to_value(doc.render_state).map_err(|e| e.to_string())
        }
        "editor.timeline" => {
            let path = params
                .get("path")
                .and_then(Value::as_str)
                .ok_or("editor.timeline requires a path")?;
            let doc = tauri::async_runtime::block_on(crate::commands::load_editor_document(
                path.to_string(),
            ))
            .map_err(|e| e.to_string())?;
            let tl =
                crate::commands::derive_project_timeline(&doc.render_state, doc.metadata.duration);
            serde_json::to_value(tl).map_err(|e| e.to_string())
        }
        "editor.zoom-regions" => {
            let path = params
                .get("path")
                .and_then(Value::as_str)
                .ok_or("editor.zoom-regions requires a path")?;
            let doc = tauri::async_runtime::block_on(crate::commands::load_editor_document(
                path.to_string(),
            ))
            .map_err(|e| e.to_string())?;
            serde_json::to_value(doc.render_state.zoom_regions).map_err(|e| e.to_string())
        }
        "editor.annotations" => {
            let path = params
                .get("path")
                .and_then(Value::as_str)
                .ok_or("editor.annotations requires a path")?;
            let doc = tauri::async_runtime::block_on(crate::commands::load_editor_document(
                path.to_string(),
            ))
            .map_err(|e| e.to_string())?;
            serde_json::to_value(doc.render_state.annotations).map_err(|e| e.to_string())
        }
        // Read-only queue inspection. `export.show` filters the same list by
        // id; the surface stays small because the queue is a single SQLite
        // table and a second method would just be SELECT * WHERE id=?.
        "export.list" => {
            let jobs = tauri::async_runtime::block_on(crate::commands::list_export_jobs(state))
                .map_err(|e| e.to_string())?;
            serde_json::to_value(jobs).map_err(|e| e.to_string())
        }
        "export.show" => {
            let id = params
                .get("id")
                .and_then(Value::as_str)
                .ok_or("export.show requires an id")?;
            let jobs = tauri::async_runtime::block_on(crate::commands::list_export_jobs(state))
                .map_err(|e| e.to_string())?;
            let job = jobs
                .into_iter()
                .find(|j| j.id == id)
                .ok_or_else(|| format!("no export job with id '{id}'"))?;
            serde_json::to_value(job).map_err(|e| e.to_string())
        }
        // Phase B write-lifecycle. `editor-session:changed` is emitted by each
        // verb that takes/releases the lock so a `recast watch editor` stream
        // sees the transition in real time.
        "editor.lock" => {
            use crate::commands::types::EditorWriterKind;
            let path_str = params
                .get("path")
                .and_then(Value::as_str)
                .ok_or("editor.lock requires a path")?;
            let path = std::path::PathBuf::from(path_str);
            let kind_str = params
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("agent");
            let kind = match kind_str {
                "ui" => EditorWriterKind::Ui,
                "agent" => EditorWriterKind::Agent,
                other => return Err(format!("editor.lock: unknown kind '{other}'")),
            };
            let writer_id = params
                .get("writerId")
                .and_then(Value::as_str)
                .ok_or("editor.lock requires a writerId")?
                .to_string();
            crate::commands::try_acquire_write(state.inner(), path, kind, writer_id)?;
            let app_clone = app.clone();
            crate::commands::persist(state.inner(), &app_clone);
            let _ = app.emit("editor-session:changed", serde_json::json!({}));
            serde_json::to_value(crate::commands::snapshot(state.inner()))
                .map_err(|e| e.to_string())
        }
        "editor.unlock" => {
            let force = params
                .get("force")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let writer_id = params.get("writerId").and_then(Value::as_str).unwrap_or("");
            let released = if force {
                let prior = crate::commands::force_release(state.inner());
                if let Some(kind) = prior {
                    log::info!("editor: force-released lock held by {kind:?}");
                }
                prior.is_some()
            } else {
                crate::commands::release_if_owner(state.inner(), writer_id)
            };
            if released {
                let app_clone = app.clone();
                crate::commands::persist(state.inner(), &app_clone);
                let _ = app.emit("editor-session:changed", serde_json::json!({}));
            }
            Ok(serde_json::json!({ "released": released }))
        }
        "editor.session" => serde_json::to_value(crate::commands::snapshot(state.inner()))
            .map_err(|e| e.to_string()),
        "editor.patch" => {
            // Replace the project's render state with the supplied JSON.
            // Validates against the source's metadata before any disk write.
            let path_str = params
                .get("path")
                .and_then(Value::as_str)
                .ok_or("editor.patch requires a path")?;
            let path = std::path::PathBuf::from(path_str);
            let writer_id = params
                .get("writerId")
                .and_then(Value::as_str)
                .ok_or("editor.patch requires a writerId")?
                .to_string();
            // Acquire (no-op if already held by the same caller).
            crate::commands::try_acquire_write(
                state.inner(),
                path.clone(),
                crate::commands::types::EditorWriterKind::Agent,
                writer_id.clone(),
            )?;
            // Load + parse the new state.
            let new_state: crate::render::graph::RenderState =
                serde_json::from_value(params.get("renderState").cloned().unwrap_or(Value::Null))
                    .map_err(|e| format!("editor.patch: invalid render state JSON: {e}"))?;
            // Probe source metadata for validation.
            let doc = tauri::async_runtime::block_on(crate::commands::load_editor_document(
                path_str.to_string(),
            ))
            .map_err(|e| e.to_string())?;
            if let Err(issues) =
                crate::commands::validate_render_state(&new_state, doc.metadata.duration)
            {
                return Err(format!(
                    "validation failed: {}",
                    serde_json::to_string(&issues).unwrap_or_else(|_| format!("{issues:?}"))
                ));
            }
            // Persist edits.json via the same path the GUI uses.
            let edits_json = serde_json::to_string(&new_state).map_err(|e| e.to_string())?;
            tauri::async_runtime::block_on(crate::commands::save_project_edits(
                path_str.to_string(),
                edits_json,
            ))
            .map_err(|e| e.to_string())?;
            let app_clone = app.clone();
            let _ = app.emit(
                "editor-state:changed",
                serde_json::json!({ "path": path_str }),
            );
            crate::commands::record_activity(state.inner());
            crate::commands::persist(state.inner(), &app_clone);
            Ok(serde_json::json!({ "applied": true }))
        }
        "editor.trim" => {
            // Small targeted edit: load current state, apply trim change,
            // validate, save.
            let path_str = params
                .get("path")
                .and_then(Value::as_str)
                .ok_or("editor.trim requires a path")?;
            let writer_id = params
                .get("writerId")
                .and_then(Value::as_str)
                .ok_or("editor.trim requires a writerId")?
                .to_string();
            let start = params
                .get("trimStart")
                .and_then(Value::as_f64)
                .ok_or("editor.trim requires trimStart")?;
            let end = params
                .get("trimEnd")
                .and_then(Value::as_f64)
                .ok_or("editor.trim requires trimEnd")?;
            let path = std::path::PathBuf::from(path_str);
            crate::commands::try_acquire_write(
                state.inner(),
                path.clone(),
                crate::commands::types::EditorWriterKind::Agent,
                writer_id.clone(),
            )?;
            let doc = tauri::async_runtime::block_on(crate::commands::load_editor_document(
                path_str.to_string(),
            ))
            .map_err(|e| e.to_string())?;
            let mut new_state = doc.render_state;
            new_state.trim_start = start;
            new_state.trim_end = end;
            if let Err(issues) =
                crate::commands::validate_render_state(&new_state, doc.metadata.duration)
            {
                return Err(format!(
                    "validation failed: {}",
                    serde_json::to_string(&issues).unwrap_or_else(|_| format!("{issues:?}"))
                ));
            }
            let edits_json = serde_json::to_string(&new_state).map_err(|e| e.to_string())?;
            tauri::async_runtime::block_on(crate::commands::save_project_edits(
                path_str.to_string(),
                edits_json,
            ))
            .map_err(|e| e.to_string())?;
            crate::commands::record_activity(state.inner());
            let app_clone = app.clone();
            crate::commands::persist(state.inner(), &app_clone);
            let _ = app.emit(
                "editor-state:changed",
                serde_json::json!({ "path": path_str }),
            );
            Ok(serde_json::json!({ "trimStart": start, "trimEnd": end }))
        }
        "editor.cut.add" => {
            let path_str = params
                .get("path")
                .and_then(Value::as_str)
                .ok_or("editor.cut.add requires a path")?;
            let writer_id = params
                .get("writerId")
                .and_then(Value::as_str)
                .ok_or("editor.cut.add requires a writerId")?
                .to_string();
            let start = params
                .get("start")
                .and_then(Value::as_f64)
                .ok_or("editor.cut.add requires start")?;
            let end = params
                .get("end")
                .and_then(Value::as_f64)
                .ok_or("editor.cut.add requires end")?;
            let path = std::path::PathBuf::from(path_str);
            crate::commands::try_acquire_write(
                state.inner(),
                path,
                crate::commands::types::EditorWriterKind::Agent,
                writer_id.clone(),
            )?;
            let doc = tauri::async_runtime::block_on(crate::commands::load_editor_document(
                path_str.to_string(),
            ))
            .map_err(|e| e.to_string())?;
            let mut new_state = doc.render_state;
            new_state.cuts.push(crate::render::graph::CutRange {
                start,
                end,
                extra: serde_json::Map::new(),
            });
            if let Err(issues) =
                crate::commands::validate_render_state(&new_state, doc.metadata.duration)
            {
                return Err(format!(
                    "validation failed: {}",
                    serde_json::to_string(&issues).unwrap_or_else(|_| format!("{issues:?}"))
                ));
            }
            let edits_json = serde_json::to_string(&new_state).map_err(|e| e.to_string())?;
            tauri::async_runtime::block_on(crate::commands::save_project_edits(
                path_str.to_string(),
                edits_json,
            ))
            .map_err(|e| e.to_string())?;
            crate::commands::record_activity(state.inner());
            let app_clone = app.clone();
            crate::commands::persist(state.inner(), &app_clone);
            let _ = app.emit(
                "editor-state:changed",
                serde_json::json!({ "path": path_str }),
            );
            Ok(serde_json::json!({ "added": { "start": start, "end": end } }))
        }
        // ---- Timeline: cuts, zoom regions, split points, segment speeds, scene animations, annotations.
        // The targeted verbs that follow each share the same shape:
        //   • look up a row by either an id (annotations) or a value match (cuts,
        //     split points, scene animations) or a positional index (zoom regions,
        //     segment speeds).
        //   • mutate, validate, persist — all routed through `patch_render_state`
        //     so the lock/validator/event semantics stay identical across verbs.
        // The result is whatever the closure returned; dispatch arms rebuild
        // the wire payload outside the closure for clarity.
        "editor.cut.list" => {
            let path_str = params
                .get("path")
                .and_then(Value::as_str)
                .ok_or("editor.cut.list requires a path")?;
            let doc = tauri::async_runtime::block_on(crate::commands::load_editor_document(
                path_str.to_string(),
            ))
            .map_err(|e| e.to_string())?;
            serde_json::to_value(
                doc.render_state
                    .cuts
                    .iter()
                    .enumerate()
                    .map(|(i, c)| serde_json::json!({ "index": i, "start": c.start, "end": c.end }))
                    .collect::<Vec<_>>(),
            )
            .map_err(|e| e.to_string())
        }
        "editor.cut.remove" => {
            let path_str = params
                .get("path")
                .and_then(Value::as_str)
                .ok_or("editor.cut.remove requires a path")?;
            let writer_id = params
                .get("writerId")
                .and_then(Value::as_str)
                .ok_or("editor.cut.remove requires a writerId")?
                .to_string();
            // Identify by index. Falls back to `(start, end)` match when an
            // `--index` wasn't provided; the validator guarantees no
            // overlap, so `start==start && end==end` is unambiguous.
            let target_index: Option<usize> = match params.get("index").cloned() {
                Some(serde_json::Value::Number(n)) => Some(
                    n.as_u64()
                        .ok_or_else(|| "--index must be an integer".to_string())?
                        as usize,
                ),
                Some(_) => return Err("--index must be an integer".into()),
                None => None,
            };
            let start_match = params.get("start").and_then(Value::as_f64);
            let end_match = params.get("end").and_then(Value::as_f64);
            crate::commands::patch_render_state(
                state.inner(),
                app,
                path_str,
                &writer_id,
                |new_state| {
                    let pos = match target_index {
                        Some(i) if i < new_state.cuts.len() => i,
                        Some(_) => return Err("cut index out of range".to_string()),
                        None => {
                            let s = start_match.ok_or(
                                "editor.cut.remove requires --index or (--start AND --end)",
                            )?;
                            let e = end_match.ok_or(
                                "editor.cut.remove requires --index or (--start AND --end)",
                            )?;
                            new_state
                                .cuts
                                .iter()
                                .position(|c| {
                                    (c.start - s).abs() < 1e-4 && (c.end - e).abs() < 1e-4
                                })
                                .ok_or_else(|| format!("no cut matching start={s}, end={e}"))?
                        }
                    };
                    let removed = new_state.cuts.remove(pos);
                    Ok(serde_json::json!({
                        "removed": { "start": removed.start, "end": removed.end }
                    }))
                },
            )
        }
        "editor.zoom.list" => {
            let path_str = params
                .get("path")
                .and_then(Value::as_str)
                .ok_or("editor.zoom.list requires a path")?;
            let doc = tauri::async_runtime::block_on(crate::commands::load_editor_document(
                path_str.to_string(),
            ))
            .map_err(|e| e.to_string())?;
            let payload: Vec<serde_json::Value> = doc
                .render_state
                .zoom_regions
                .iter()
                .enumerate()
                .map(|(i, z)| {
                    serde_json::json!({
                        "index": i,
                        "start": z.start,
                        "end": z.end,
                        "scale": z.scale,
                        "centerX": z.center_x,
                        "centerY": z.center_y,
                        "rampIn": z.ramp_in,
                        "rampOut": z.ramp_out,
                        "hidden": z.hidden,
                    })
                })
                .collect();
            serde_json::to_value(payload).map_err(|e| e.to_string())
        }
        "editor.zoom.add" => {
            let path_str = params
                .get("path")
                .and_then(Value::as_str)
                .ok_or("editor.zoom.add requires a path")?;
            let writer_id = params
                .get("writerId")
                .and_then(Value::as_str)
                .ok_or("editor.zoom.add requires a writerId")?
                .to_string();
            let start = params.get("start").and_then(Value::as_f64).ok_or("start")?;
            let end = params.get("end").and_then(Value::as_f64).ok_or("end")?;
            let scale = params.get("scale").and_then(Value::as_f64).ok_or("scale")?;
            let center_x = params.get("centerX").and_then(Value::as_f64).unwrap_or(0.5);
            let center_y = params.get("centerY").and_then(Value::as_f64).unwrap_or(0.5);
            let ramp_in = params.get("rampIn").and_then(Value::as_f64).unwrap_or(0.35);
            let ramp_out = params
                .get("rampOut")
                .and_then(Value::as_f64)
                .unwrap_or(0.35);
            let hidden = params
                .get("hidden")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            crate::commands::patch_render_state(
                state.inner(),
                app,
                path_str,
                &writer_id,
                |new_state| {
                    use crate::render::easing::Easing;
                    let region = crate::render::node_types::ZoomRegion {
                        start,
                        end,
                        scale,
                        ease_in: Easing::default(),
                        ease_out: Easing::default(),
                        ramp_in,
                        ramp_out,
                        center_x,
                        center_y,
                        hidden,
                        motion_blur: 0.0,
                        extra: serde_json::Map::new(),
                    };
                    let index = new_state.zoom_regions.len();
                    new_state.zoom_regions.push(region);
                    Ok(serde_json::json!({ "index": index, "start": start, "end": end }))
                },
            )
        }
        "editor.zoom.remove" => {
            let path_str = params
                .get("path")
                .and_then(Value::as_str)
                .ok_or("editor.zoom.remove requires a path")?;
            let writer_id = params
                .get("writerId")
                .and_then(Value::as_str)
                .ok_or("editor.zoom.remove requires a writerId")?
                .to_string();
            let index = params
                .get("index")
                .and_then(Value::as_u64)
                .ok_or("editor.zoom.remove requires --index")? as usize;
            crate::commands::patch_render_state(
                state.inner(),
                app,
                path_str,
                &writer_id,
                |new_state| {
                    if index >= new_state.zoom_regions.len() {
                        return Err("zoom index out of range".to_string());
                    }
                    let removed = new_state.zoom_regions.remove(index);
                    Ok(serde_json::json!({
                        "removed": { "start": removed.start, "end": removed.end }
                    }))
                },
            )
        }
        "editor.split-point.list" => {
            let path_str = params
                .get("path")
                .and_then(Value::as_str)
                .ok_or("editor.split-point.list requires a path")?;
            let doc = tauri::async_runtime::block_on(crate::commands::load_editor_document(
                path_str.to_string(),
            ))
            .map_err(|e| e.to_string())?;
            serde_json::to_value(doc.render_state.split_points).map_err(|e| e.to_string())
        }
        "editor.split-point.add" => {
            let path_str = params
                .get("path")
                .and_then(Value::as_str)
                .ok_or("editor.split-point.add requires a path")?;
            let writer_id = params
                .get("writerId")
                .and_then(Value::as_str)
                .ok_or("editor.split-point.add requires a writerId")?
                .to_string();
            let at = params
                .get("at")
                .and_then(Value::as_f64)
                .ok_or("editor.split-point.add requires --at")?;
            crate::commands::patch_render_state(
                state.inner(),
                app,
                path_str,
                &writer_id,
                |new_state| {
                    if !new_state.split_points.contains(&at) {
                        new_state.split_points.push(at);
                        new_state
                            .split_points
                            .sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                    }
                    Ok(serde_json::json!({ "added": at }))
                },
            )
        }
        "editor.split-point.remove" => {
            let path_str = params
                .get("path")
                .and_then(Value::as_str)
                .ok_or("editor.split-point.remove requires a path")?;
            let writer_id = params
                .get("writerId")
                .and_then(Value::as_str)
                .ok_or("editor.split-point.remove requires a writerId")?
                .to_string();
            let at = params
                .get("at")
                .and_then(Value::as_f64)
                .ok_or("editor.split-point.remove requires --at")?;
            crate::commands::patch_render_state(
                state.inner(),
                app,
                path_str,
                &writer_id,
                |new_state| {
                    let before = new_state.split_points.len();
                    new_state.split_points.retain(|x| (*x - at).abs() > 1e-4);
                    if new_state.split_points.len() == before {
                        return Err(format!("no split point at {at}"));
                    }
                    Ok(serde_json::json!({ "removed": at }))
                },
            )
        }
        "editor.speed.list" => {
            let path_str = params
                .get("path")
                .and_then(Value::as_str)
                .ok_or("editor.speed.list requires a path")?;
            let doc = tauri::async_runtime::block_on(crate::commands::load_editor_document(
                path_str.to_string(),
            ))
            .map_err(|e| e.to_string())?;
            serde_json::to_value(doc.render_state.segment_speeds).map_err(|e| e.to_string())
        }
        "editor.speed.set" => {
            let path_str = params
                .get("path")
                .and_then(Value::as_str)
                .ok_or("editor.speed.set requires a path")?;
            let writer_id = params
                .get("writerId")
                .and_then(Value::as_str)
                .ok_or("editor.speed.set requires a writerId")?
                .to_string();
            let segment_start = params
                .get("segmentStart")
                .and_then(Value::as_f64)
                .ok_or("editor.speed.set requires --segment-start")?;
            let rate = params
                .get("rate")
                .and_then(Value::as_f64)
                .ok_or("editor.speed.set requires --rate")?;
            crate::commands::patch_render_state(
                state.inner(),
                app,
                path_str,
                &writer_id,
                |new_state| {
                    let pos = new_state
                        .segment_speeds
                        .iter()
                        .position(|s| (s.start - segment_start).abs() < 1e-4);
                    match pos {
                        Some(i) => new_state.segment_speeds[i].speed = rate,
                        None => new_state
                            .segment_speeds
                            .push(crate::render::graph::SegmentSpeed {
                                start: segment_start,
                                speed: rate,
                            }),
                    }
                    Ok(serde_json::json!({ "segmentStart": segment_start, "rate": rate }))
                },
            )
        }
        "editor.speed.remove" => {
            let path_str = params
                .get("path")
                .and_then(Value::as_str)
                .ok_or("editor.speed.remove requires a path")?;
            let writer_id = params
                .get("writerId")
                .and_then(Value::as_str)
                .ok_or("editor.speed.remove requires a writerId")?
                .to_string();
            let segment_start = params
                .get("segmentStart")
                .and_then(Value::as_f64)
                .ok_or("editor.speed.remove requires --segment-start")?;
            crate::commands::patch_render_state(
                state.inner(),
                app,
                path_str,
                &writer_id,
                |new_state| {
                    let before = new_state.segment_speeds.len();
                    new_state
                        .segment_speeds
                        .retain(|s| (s.start - segment_start).abs() > 1e-4);
                    if new_state.segment_speeds.len() == before {
                        return Err(format!(
                            "no speed override at segment start {segment_start}"
                        ));
                    }
                    Ok(serde_json::json!({ "removed": segment_start }))
                },
            )
        }
        "editor.annotations.list" => {
            let path_str = params
                .get("path")
                .and_then(Value::as_str)
                .ok_or("editor.annotations.list requires a path")?;
            let doc = tauri::async_runtime::block_on(crate::commands::load_editor_document(
                path_str.to_string(),
            ))
            .map_err(|e| e.to_string())?;
            let payload: Vec<serde_json::Value> = doc
                .render_state
                .annotations
                .iter()
                .map(|a| serde_json::json!({ "id": a.id, "kind": annotation_kind_name(&a.kind) }))
                .collect();
            serde_json::to_value(payload).map_err(|e| e.to_string())
        }
        "editor.annotations.add" => {
            let path_str = params
                .get("path")
                .and_then(Value::as_str)
                .ok_or("editor.annotations.add requires a path")?;
            let writer_id = params
                .get("writerId")
                .and_then(Value::as_str)
                .ok_or("editor.annotations.add requires a writerId")?
                .to_string();
            let kind_name = params
                .get("kind")
                .and_then(Value::as_str)
                .ok_or("editor.annotations.add requires --kind")?
                .to_string();
            let geometry = params
                .get("geometry")
                .cloned()
                .ok_or("editor.annotations.add requires --geometry <JSON>")?;
            let id = params
                .get("id")
                .and_then(Value::as_str)
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    format!(
                        "{}-{}",
                        kind_name,
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_millis())
                            .unwrap_or(0)
                    )
                });
            let start = params
                .get("start")
                .and_then(Value::as_f64)
                .ok_or("editor.annotations.add requires --start")?;
            let end = params
                .get("end")
                .and_then(Value::as_f64)
                .ok_or("editor.annotations.add requires --end")?;
            let opacity = params.get("opacity").and_then(Value::as_f64).unwrap_or(1.0);
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .map(|s| s.to_string());
            let z_index = params
                .get("z")
                .and_then(Value::as_i64)
                .map(|n| n as i32)
                .unwrap_or(0);
            crate::commands::patch_render_state(
                state.inner(),
                app,
                path_str,
                &writer_id,
                |new_state| {
                    let kind = build_annotation_kind(&kind_name, &geometry)?;
                    let annotation = crate::render::node_types::Annotation {
                        id: id.clone(),
                        start,
                        end,
                        ramp_in: 0.2,
                        ramp_out: 0.2,
                        ease_in: Default::default(),
                        ease_out: Default::default(),
                        stroke: Default::default(),
                        fill: "rgba(59,130,246,0.20)".into(),
                        kind,
                        name: name.clone(),
                        z_index,
                        locked: false,
                        hidden: false,
                        opacity,
                        glow: None,
                        anchor: crate::render::node_types::AnnotationAnchor::default(),
                    };
                    new_state.annotations.push(annotation);
                    Ok(serde_json::json!({ "id": id }))
                },
            )
        }
        "editor.annotations.update" => {
            let path_str = params
                .get("path")
                .and_then(Value::as_str)
                .ok_or("editor.annotations.update requires a path")?;
            let writer_id = params
                .get("writerId")
                .and_then(Value::as_str)
                .ok_or("editor.annotations.update requires a writerId")?
                .to_string();
            let id = params
                .get("id")
                .and_then(Value::as_str)
                .ok_or("editor.annotations.update requires --id")?
                .to_string();
            let patch_obj = params
                .get("patch")
                .and_then(Value::as_object)
                .ok_or("editor.annotations.update requires --patch <JSON object>")?
                .clone();
            crate::commands::patch_render_state(
                state.inner(),
                app,
                path_str,
                &writer_id,
                |new_state| {
                    let pos = new_state
                        .annotations
                        .iter()
                        .position(|a| a.id == id)
                        .ok_or_else(|| format!("no annotation with id '{id}'"))?;
                    let annotation_json = serde_json::to_value(&new_state.annotations[pos])
                        .map_err(|e| format!("serialize: {e}"))?;
                    let mut merged = annotation_json;
                    if let Some(obj) = merged.as_object_mut() {
                        for (k, v) in patch_obj.iter() {
                            obj.insert(k.clone(), v.clone());
                        }
                    }
                    let updated: crate::render::node_types::Annotation =
                        serde_json::from_value(merged)
                            .map_err(|e| format!("patch produced invalid annotation: {e}"))?;
                    new_state.annotations[pos] = updated;
                    Ok(serde_json::json!({ "id": id }))
                },
            )
        }
        "editor.annotations.remove" => {
            let path_str = params
                .get("path")
                .and_then(Value::as_str)
                .ok_or("editor.annotations.remove requires a path")?;
            let writer_id = params
                .get("writerId")
                .and_then(Value::as_str)
                .ok_or("editor.annotations.remove requires a writerId")?
                .to_string();
            let id = params
                .get("id")
                .and_then(Value::as_str)
                .ok_or("editor.annotations.remove requires --id")?
                .to_string();
            crate::commands::patch_render_state(
                state.inner(),
                app,
                path_str,
                &writer_id,
                |new_state| {
                    let before = new_state.annotations.len();
                    new_state.annotations.retain(|a| a.id != id);
                    if new_state.annotations.len() == before {
                        return Err(format!("no annotation with id '{id}'"));
                    }
                    Ok(serde_json::json!({ "removed": id }))
                },
            )
        }
        "editor.animations.list" => {
            let path_str = params
                .get("path")
                .and_then(Value::as_str)
                .ok_or("editor.animations.list requires a path")?;
            let doc = tauri::async_runtime::block_on(crate::commands::load_editor_document(
                path_str.to_string(),
            ))
            .map_err(|e| e.to_string())?;
            let payload: Vec<serde_json::Value> = doc
                .render_state
                .scene_animations
                .iter()
                .map(|a| {
                    serde_json::json!({
                        "start": a.start,
                        "in": a.anim_in.as_ref().map(spec_to_json),
                        "out": a.anim_out.as_ref().map(spec_to_json),
                    })
                })
                .collect();
            serde_json::to_value(payload).map_err(|e| e.to_string())
        }
        "editor.animations.remove" => {
            let path_str = params
                .get("path")
                .and_then(Value::as_str)
                .ok_or("editor.animations.remove requires a path")?;
            let writer_id = params
                .get("writerId")
                .and_then(Value::as_str)
                .ok_or("editor.animations.remove requires a writerId")?
                .to_string();
            let start = params
                .get("start")
                .and_then(Value::as_f64)
                .ok_or("editor.animations.remove requires --start")?;
            crate::commands::patch_render_state(
                state.inner(),
                app,
                path_str,
                &writer_id,
                |new_state| {
                    let before = new_state.scene_animations.len();
                    new_state
                        .scene_animations
                        .retain(|a| (a.start - start).abs() > 1e-4);
                    if new_state.scene_animations.len() == before {
                        return Err(format!("no scene animation at segment start {start}"));
                    }
                    Ok(serde_json::json!({ "removed": start }))
                },
            )
        }
        "editor.animations.add" => {
            let path_str = params
                .get("path")
                .and_then(Value::as_str)
                .ok_or("editor.animations.add requires a path")?;
            let writer_id = params
                .get("writerId")
                .and_then(Value::as_str)
                .ok_or("editor.animations.add requires a writerId")?
                .to_string();
            let start = params
                .get("start")
                .and_then(Value::as_f64)
                .ok_or("editor.animations.add requires --start")?;
            let in_spec = params.get("in").cloned();
            let out_spec = params.get("out").cloned();
            crate::commands::patch_render_state(
                state.inner(),
                app,
                path_str,
                &writer_id,
                |new_state| {
                    // Replace any existing animation at this start; otherwise push.
                    let anim_in: Option<crate::render::scene_anim::SceneAnimSpec> = match in_spec {
                        Some(v) => Some(
                            serde_json::from_value(v)
                                .map_err(|e| format!("invalid --in spec: {e}"))?,
                        ),
                        None => None,
                    };
                    let anim_out: Option<crate::render::scene_anim::SceneAnimSpec> = match out_spec
                    {
                        Some(v) => Some(
                            serde_json::from_value(v)
                                .map_err(|e| format!("invalid --out spec: {e}"))?,
                        ),
                        None => None,
                    };
                    let anim = crate::render::scene_anim::SegmentAnim {
                        start,
                        anim_in,
                        anim_out,
                    };
                    if let Some(pos) = new_state
                        .scene_animations
                        .iter()
                        .position(|a| (a.start - start).abs() < 1e-4)
                    {
                        new_state.scene_animations[pos] = anim;
                    } else {
                        new_state.scene_animations.push(anim);
                    }
                    Ok(serde_json::json!({ "start": start }))
                },
            )
        }
        // Universal mutator: any scalar/struct field via dotted JSON pointer.
        // Used as the escape hatch for every field that doesn't get its own
        // targeted verb. `--value` accepts a JSON value (string for strings,
        // number, true/false, array, object).
        "editor.set" => {
            let path_str = params
                .get("path")
                .and_then(Value::as_str)
                .ok_or("editor.set requires a path")?;
            let writer_id = params
                .get("writerId")
                .and_then(Value::as_str)
                .ok_or("editor.set requires a writerId")?
                .to_string();
            let field = params
                .get("field")
                .and_then(Value::as_str)
                .ok_or("editor.set requires --field")?
                .to_string();
            let value = params
                .get("value")
                .cloned()
                .ok_or("editor.set requires --value")?;
            crate::commands::patch_render_state(
                state.inner(),
                app,
                path_str,
                &writer_id,
                |new_state| {
                    let mut state_json =
                        serde_json::to_value(&*new_state).map_err(|e| e.to_string())?;
                    crate::commands::apply_dotted_path_set(&mut state_json, &field, value)?;
                    let updated: RenderState = serde_json::from_value(state_json)
                        .map_err(|e| format!("set: invalid value at '{field}': {e}"))?;
                    *new_state = updated;
                    Ok(serde_json::json!({ "applied": true, "field": field }))
                },
            )
        }
        "export.start" => {
            let path_str = params
                .get("path")
                .and_then(Value::as_str)
                .ok_or("export.start requires a path")?;
            let writer_id = params
                .get("writerId")
                .and_then(Value::as_str)
                .ok_or("export.start requires a writerId")?
                .to_string();
            let format = params
                .get("format")
                .and_then(Value::as_str)
                .unwrap_or("mp4")
                .to_string();
            let quality = params
                .get("quality")
                .and_then(Value::as_str)
                .unwrap_or("balanced")
                .to_string();
            let speed = params
                .get("speed")
                .and_then(Value::as_str)
                .map(|s| s.to_string());
            let fps = params.get("fps").and_then(Value::as_f64);
            let burn_captions = params
                .get("burnCaptions")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let caption_sidecar = match params.get("captionSidecar") {
                Some(Value::String(fmt)) if fmt == "vtt" || fmt == "srt" => {
                    Some(crate::commands::types::CaptionSidecar {
                        format: fmt.to_string(),
                        transcript: doc_transcript(state.inner(), path_str, fmt)?,
                    })
                }
                Some(Value::String(other)) => {
                    return Err(format!(
                        "captionSidecar must be 'vtt', 'srt', or absent; got '{other}'"
                    ));
                }
                Some(Value::Null) | None => None,
                _ => return Err("captionSidecar must be a string".into()),
            };
            let gif_settings = if format == "gif" {
                let mut s = crate::commands::types::GifSettings::default();
                if let Some(n) = params.get("gifFps").and_then(Value::as_u64) {
                    s.fps = Some(n as u32);
                }
                if let Some(q) = params.get("gifQuality").and_then(Value::as_str) {
                    if !matches!(q, "low" | "medium" | "high") {
                        return Err(format!("gifQuality must be low|medium|high; got '{q}'"));
                    }
                    s.quality = q.to_string();
                }
                if let Some(loop_v) = params.get("gifLoop") {
                    s.r#loop = loop_v.clone();
                }
                if let Some(d) = params.get("gifDither").and_then(Value::as_str) {
                    if !matches!(d, "bayer" | "sierra2" | "none") {
                        return Err(format!("gifDither must be bayer|sierra2|none; got '{d}'"));
                    }
                    s.dither = d.to_string();
                }
                Some(s)
            } else {
                None
            };
            // Validate `--speed` against the same enum the encode uses
            // (`fast` | `balanced` | `quality`), if supplied.
            if let Some(ref s) = speed {
                if !matches!(s.as_str(), "fast" | "balanced" | "quality") {
                    return Err(format!(
                        "export.start: --speed must be fast|balanced|quality; got '{s}'"
                    ));
                }
            }
            let writer_id_for_lock = writer_id.clone();
            let path_buf = std::path::PathBuf::from(path_str);
            crate::commands::try_acquire_write(
                state.inner(),
                path_buf,
                crate::commands::types::EditorWriterKind::Agent,
                writer_id_for_lock,
            )?;
            let doc = tauri::async_runtime::block_on(crate::commands::load_editor_document(
                path_str.to_string(),
            ))
            .map_err(|e| e.to_string())?;
            let render_state = match params.get("renderState") {
                Some(v) => serde_json::from_value(v.clone())
                    .map_err(|e| format!("export.start: invalid renderState: {e}"))?,
                None => doc.render_state,
            };
            if let Err(issues) =
                crate::commands::validate_render_state(&render_state, doc.metadata.duration)
            {
                return Err(format!(
                    "validation failed: {}",
                    serde_json::to_string(&issues).unwrap_or_else(|_| format!("{issues:?}"))
                ));
            }
            let request = crate::commands::types::ExportRequest {
                export_id: format!("cli-{}-{}", std::process::id(), now_unix_ms()),
                input_path: path_str.to_string(),
                format,
                quality,
                speed,
                render_state,
                gif_settings,
                fps,
                burn_captions,
                caption_sidecar,
            };
            tauri::async_runtime::block_on(crate::commands::enqueue_export(
                app.clone(),
                request.clone(),
                state.clone(),
            ))
            .map_err(|e| e.to_string())?;
            crate::commands::record_activity(state.inner());
            Ok(serde_json::json!({
                "exportId": request.export_id,
                "format": request.format,
                "quality": request.quality,
                "speed": request.speed,
                "fps": request.fps,
                "burnCaptions": request.burn_captions,
                "gifSettings": request.gif_settings,
            }))
        }
        "export.cancel" => {
            let id = params
                .get("id")
                .and_then(Value::as_str)
                .ok_or("export.cancel requires an id")?;
            tauri::async_runtime::block_on(crate::commands::cancel_export_job(
                app.clone(),
                id.to_string(),
                state.clone(),
            ))
            .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "cancelled": id }))
        }
        other => Err(format!("unknown method: {other}")),
    }
}

fn now_unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Resolve the project's recorded transcript for a `CaptionSidecar`.
///
/// This is intentionally a no-op stub for now — the GUI export path is
/// the canonical reader, and the CLI sidecar surfaces mirror only when
/// a transcript exists. Callers should pass `--burn-captions` or
/// `--caption-sidecar` only after verifying via `recast project show`
/// that the project has a `transcript` populated in its render state.
fn doc_transcript(
    _state: &crate::commands::types::AppState,
    _path: &str,
    _format: &str,
) -> Result<crate::transcription::Transcript, String> {
    Ok(crate::transcription::Transcript {
        engine: String::new(),
        model_id: String::new(),
        segments: Vec::new(),
        language: None,
    })
}

/// Map an `AnnotationKind` to its discriminant string ("rect", "ellipse",
/// "arrow", "image", "blur", "text", "unsupported"). Used by the agent
/// surface so a script can match by name without re-deriving the JSON
/// shape.
fn annotation_kind_name(kind: &crate::render::node_types::AnnotationKind) -> &'static str {
    use crate::render::node_types::AnnotationKind::*;
    match kind {
        Rect { .. } => "rect",
        Ellipse { .. } => "ellipse",
        Arrow { .. } => "arrow",
        Image { .. } => "image",
        Blur { .. } => "blur",
        Text { .. } => "text",
        Unsupported => "unsupported",
    }
}

/// Construct an `AnnotationKind` from a `--kind` discriminant + a `--geometry`
/// JSON object. Keeps the per-kind geometry surface flat at the CLI edge so
/// new kinds don't require new dispatch arms.
fn build_annotation_kind(
    kind_name: &str,
    geometry: &serde_json::Value,
) -> Result<crate::render::node_types::AnnotationKind, String> {
    use crate::render::node_types::AnnotationKind;
    let require_geom =
        |missing: &str| -> Result<serde_json::Map<String, serde_json::Value>, String> {
            geometry
                .as_object()
                .cloned()
                .ok_or_else(|| format!("annotation --geometry must be a JSON object ({missing})"))
        };
    match kind_name {
        "rect" => {
            let obj = require_geom("need x, y, w, h")?;
            Ok(AnnotationKind::Rect {
                x: obj.get("x").and_then(Value::as_f64).ok_or("rect needs x")?,
                y: obj.get("y").and_then(Value::as_f64).ok_or("rect needs y")?,
                w: obj.get("w").and_then(Value::as_f64).ok_or("rect needs w")?,
                h: obj.get("h").and_then(Value::as_f64).ok_or("rect needs h")?,
                radius: obj.get("radius").and_then(Value::as_f64).unwrap_or(0.0),
            })
        }
        "ellipse" => {
            let obj = require_geom("need x, y, w, h")?;
            Ok(AnnotationKind::Ellipse {
                x: obj
                    .get("x")
                    .and_then(Value::as_f64)
                    .ok_or("ellipse needs x")?,
                y: obj
                    .get("y")
                    .and_then(Value::as_f64)
                    .ok_or("ellipse needs y")?,
                w: obj
                    .get("w")
                    .and_then(Value::as_f64)
                    .ok_or("ellipse needs w")?,
                h: obj
                    .get("h")
                    .and_then(Value::as_f64)
                    .ok_or("ellipse needs h")?,
            })
        }
        "arrow" => {
            let obj = require_geom("need x1, y1, x2, y2")?;
            Ok(AnnotationKind::Arrow {
                x1: obj
                    .get("x1")
                    .and_then(Value::as_f64)
                    .ok_or("arrow needs x1")?,
                y1: obj
                    .get("y1")
                    .and_then(Value::as_f64)
                    .ok_or("arrow needs y1")?,
                x2: obj
                    .get("x2")
                    .and_then(Value::as_f64)
                    .ok_or("arrow needs x2")?,
                y2: obj
                    .get("y2")
                    .and_then(Value::as_f64)
                    .ok_or("arrow needs y2")?,
                head_size: obj.get("headSize").and_then(Value::as_f64).unwrap_or(0.2),
            })
        }
        "blur" => {
            let obj = require_geom("need x, y, w, h")?;
            Ok(AnnotationKind::Blur {
                x: obj.get("x").and_then(Value::as_f64).ok_or("blur needs x")?,
                y: obj.get("y").and_then(Value::as_f64).ok_or("blur needs y")?,
                w: obj.get("w").and_then(Value::as_f64).ok_or("blur needs w")?,
                h: obj.get("h").and_then(Value::as_f64).ok_or("blur needs h")?,
                strength: obj.get("strength").and_then(Value::as_f64).unwrap_or(0.5),
                variant: obj
                    .get("variant")
                    .and_then(Value::as_str)
                    .unwrap_or("solid")
                    .to_string(),
                tint_color: obj
                    .get("tintColor")
                    .and_then(Value::as_str)
                    .unwrap_or("#000000")
                    .to_string(),
                radius: obj.get("radius").and_then(Value::as_f64).unwrap_or(0.0),
            })
        }
        "image" => {
            let obj = require_geom("need x, y, w, h, path")?;
            Ok(AnnotationKind::Image {
                x: obj
                    .get("x")
                    .and_then(Value::as_f64)
                    .ok_or("image needs x")?,
                y: obj
                    .get("y")
                    .and_then(Value::as_f64)
                    .ok_or("image needs y")?,
                w: obj
                    .get("w")
                    .and_then(Value::as_f64)
                    .ok_or("image needs w")?,
                h: obj
                    .get("h")
                    .and_then(Value::as_f64)
                    .ok_or("image needs h")?,
                path: obj
                    .get("path")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                opacity: obj.get("opacity").and_then(Value::as_f64).unwrap_or(1.0),
                radius: obj.get("radius").and_then(Value::as_f64).unwrap_or(0.0),
            })
        }
        "text" => {
            let obj = require_geom("need x, y, w, h")?;
            Ok(AnnotationKind::Text {
                x: obj.get("x").and_then(Value::as_f64).ok_or("text needs x")?,
                y: obj.get("y").and_then(Value::as_f64).ok_or("text needs y")?,
                w: obj.get("w").and_then(Value::as_f64).ok_or("text needs w")?,
                h: obj.get("h").and_then(Value::as_f64).ok_or("text needs h")?,
                content: obj
                    .get("content")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                font_family: obj
                    .get("fontFamily")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                font_size: obj.get("fontSize").and_then(Value::as_f64).unwrap_or(16.0),
                font_weight: obj
                    .get("fontWeight")
                    .and_then(Value::as_f64)
                    .unwrap_or(400.0),
                color: obj
                    .get("color")
                    .and_then(Value::as_str)
                    .unwrap_or("#ffffff")
                    .to_string(),
                align: obj
                    .get("align")
                    .and_then(Value::as_str)
                    .unwrap_or("left")
                    .to_string(),
                line_height: obj.get("lineHeight").and_then(Value::as_f64).unwrap_or(1.2),
            })
        }
        other => Err(format!("unknown annotation kind '{other}'")),
    }
}

/// Render a `SceneAnimSpec` as the JSON object the GUI/agent see on disk.
fn spec_to_json(spec: &crate::render::scene_anim::SceneAnimSpec) -> serde_json::Value {
    serde_json::json!({
        "kind": spec.kind,
        "durationMs": spec.duration_ms,
        "easing": spec.easing,
        "dir": spec.dir,
        "intensity": spec.intensity,
    })
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
