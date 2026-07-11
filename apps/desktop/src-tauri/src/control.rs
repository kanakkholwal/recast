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

    for incoming in listener.incoming() {
        match incoming {
            Ok(mut stream) => {
                if let Err(e) = handle_conn(app, &mut stream, &token) {
                    log::debug!("cli control connection error: {e}");
                }
            }
            Err(e) => log::debug!("cli control accept error: {e}"),
        }
    }
    Ok(())
}

fn handle_conn(app: &tauri::AppHandle, stream: &mut Stream, token: &str) -> Result<(), String> {
    let mut line = String::new();
    {
        let mut reader = BufReader::new(&mut *stream);
        reader.read_line(&mut line).map_err(|e| e.to_string())?;
    }

    let response = match serde_json::from_str::<Request>(&line) {
        Ok(req) if req.token == token => match dispatch(app, &req.method, req.params) {
            Ok(result) => Response {
                ok: true,
                result: Some(result),
                error: None,
            },
            Err(error) => Response {
                ok: false,
                result: None,
                error: Some(error),
            },
        },
        Ok(_) => Response {
            ok: false,
            result: None,
            error: Some("unauthorized".into()),
        },
        Err(e) => Response {
            ok: false,
            result: None,
            error: Some(format!("bad request: {e}")),
        },
    };

    let mut out = serde_json::to_string(&response).map_err(|e| e.to_string())?;
    out.push('\n');
    stream
        .write_all(out.as_bytes())
        .map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())?;
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
        "rec.start" => {
            let p: StartParams = serde_json::from_value(params).map_err(|e| e.to_string())?;
            let result = tauri::async_runtime::block_on(crate::commands::start_recording(
                p.target_type,
                p.target_id,
                p.region,
                p.options,
                state,
            ))
            .map_err(|e| e.to_string())?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        "rec.stop" => {
            let path = tauri::async_runtime::block_on(crate::commands::stop_recording(state))
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
        other => Err(format!("unknown method: {other}")),
    }
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
