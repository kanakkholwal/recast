//! Headless CLI branch. Parsed in `main` before the Tauri app boots so agents
//! and scripts can drive Recast without a running window.
//!
//! Enumeration verbs reuse the same functions the UI invokes; control verbs
//! (status, rec) talk to a running app over the local socket (see control.rs).
//! Output is YAML at a terminal and JSON when piped or captured, overridable
//! with `--format`. See docs/cli-automation-plan.md.

use std::io::IsTerminal;

use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use serde_json::{json, Value};

use crate::commands::system::{AudioDeviceInfo, CaptureCapabilities};
use crate::commands::types::{CameraDeviceInfo, CameraValidationResult, DisplayInfo, WindowInfo};

/// Verbs that route to the headless CLI. Anything else (a `.recast` path, a
/// `recast://` URL, `--new-recording`, or a bare launch) falls through to the
/// GUI path untouched. Kept deliberately narrow so file-association and
/// deep-link launches are never intercepted.
const CLI_VERBS: &[&str] = &[
    "devices",
    "displays",
    "windows",
    "mics",
    "cameras",
    "capabilities",
    "doctor",
    "status",
    "rec",
    "select",
    "set",
    "selection",
    "install",
    "uninstall",
];

/// True when argv[1] is a CLI verb or a help request. `main` uses this to pick
/// the headless branch over the GUI launch.
pub fn should_handle() -> bool {
    match std::env::args().nth(1).as_deref() {
        Some(first) => CLI_VERBS.contains(&first) || matches!(first, "-h" | "--help" | "help"),
        None => false,
    }
}

/// Run the CLI to completion and exit the process. Never returns.
pub fn run_and_exit() -> ! {
    #[cfg(windows)]
    attach_parent_console();

    let cli = Cli::parse();
    let code = match dispatch(&cli) {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("recast: {err}");
            1
        }
    };
    std::process::exit(code);
}

/// Rendered output format. Data is always modelled as JSON internally; this
/// only controls how it is printed.
#[derive(Clone, Copy, ValueEnum)]
enum Format {
    /// Human-readable, the default at a terminal.
    Yaml,
    /// Machine-readable single line, the default when piped or captured.
    Json,
}

#[derive(Parser)]
#[command(
    name = "recast",
    about = "Recast recorder automation CLI",
    disable_help_subcommand = true
)]
struct Cli {
    /// Output format. Defaults to YAML at a terminal, JSON when piped or captured.
    #[arg(long, short = 'f', global = true, value_enum)]
    format: Option<Format>,
    /// Include base64 thumbnails in display/window output (large, off by default).
    #[arg(long, global = true)]
    thumbnails: bool,
    /// Do not auto-start Recast for control commands; fail if it is not running.
    #[arg(long, global = true)]
    no_launch: bool,
    /// Milliseconds to wait for the app + control server on auto-launch.
    #[arg(long, global = true, default_value_t = 8000)]
    timeout_ms: u64,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Displays, windows, mics and cameras in one payload.
    Devices {
        #[command(subcommand)]
        action: ListAction,
    },
    /// Connected displays/monitors.
    Displays {
        #[command(subcommand)]
        action: ListAction,
    },
    /// Capturable application windows.
    Windows {
        #[command(subcommand)]
        action: WindowsAction,
    },
    /// Microphone input devices.
    Mics {
        #[command(subcommand)]
        action: ListAction,
    },
    /// Camera / webcam devices.
    Cameras {
        #[command(subcommand)]
        action: CamerasAction,
    },
    /// Capture backends supported on this OS.
    Capabilities,
    /// Capabilities plus a one-line readiness summary.
    Doctor,
    /// Whether the app is running plus its recording state.
    Status,
    /// Recording lifecycle. Talks to a running app (auto-launches by default).
    Rec {
        #[command(subcommand)]
        action: RecAction,
    },
    /// Stage the source/mic/camera for the next recording (the capture intent).
    Select {
        #[command(subcommand)]
        action: SelectAction,
    },
    /// Tweak capture options on the staged intent.
    Set {
        #[command(subcommand)]
        action: SetAction,
    },
    /// Inspect or reset the staged capture intent.
    Selection {
        #[command(subcommand)]
        action: SelectionAction,
    },
    /// Put `recast` on your PATH so it runs as a bare command in any terminal.
    Install,
    /// Remove `recast` from your PATH.
    Uninstall,
}

#[derive(Subcommand)]
enum RecAction {
    /// Start a recording.
    Start(StartArgs),
    /// Stop the recording and print the saved project path.
    Stop,
    /// Pause the recording.
    Pause,
    /// Resume a paused recording.
    Resume,
    /// Print the current recording/paused state.
    Status,
}

#[derive(Subcommand)]
enum SelectAction {
    /// Record a display by id (from `displays list`).
    Screen { id: u32 },
    /// Record a window by id (from `windows list`).
    Window { id: u32 },
    /// Record a region given as X,Y,W,H in physical pixels.
    Region {
        #[arg(value_name = "X,Y,W,H")]
        spec: String,
    },
    /// Microphone: a device id, `default`, or `none`.
    Mic {
        #[arg(value_name = "ID|default|none")]
        value: String,
    },
    /// Camera: a device id or `none`.
    Camera {
        #[arg(value_name = "ID|none")]
        value: String,
    },
}

#[derive(Subcommand)]
enum SetAction {
    /// System audio capture, `on` or `off`.
    SystemAudio {
        #[arg(value_name = "ON|OFF")]
        value: String,
    },
    /// Capture frame rate.
    Fps { value: u32 },
    /// Encode quality: auto, balanced, high, or pristine.
    Quality { value: String },
    /// Pre-roll countdown in seconds, or `off`.
    Countdown {
        #[arg(value_name = "SECONDS|off")]
        value: String,
    },
}

#[derive(Subcommand)]
enum SelectionAction {
    /// Print the staged capture intent.
    Show,
    /// Reset the intent to defaults (no source, system audio on).
    Reset,
}

#[derive(clap::Args)]
struct StartArgs {
    /// Record a display by id (from `displays list`).
    #[arg(long, group = "target")]
    screen: Option<u32>,
    /// Record a window by id (from `windows list`).
    #[arg(long, group = "target")]
    window: Option<u32>,
    /// Record a region as X,Y,W,H in physical pixels.
    #[arg(long, group = "target", value_name = "X,Y,W,H")]
    region: Option<String>,
    /// Microphone: a device id, `default`, or `none` (default none).
    #[arg(long)]
    mic: Option<String>,
    /// Camera: a device id or `none` (default none).
    #[arg(long)]
    camera: Option<String>,
    /// System audio capture, `on` or `off` (default on).
    #[arg(long = "system-audio", value_name = "ON|OFF")]
    system_audio: Option<String>,
    /// Capture frame rate.
    #[arg(long)]
    fps: Option<u32>,
    /// Encode quality: auto, balanced, high, or pristine.
    #[arg(long)]
    quality: Option<String>,
}

#[derive(Subcommand)]
enum ListAction {
    /// List the sources.
    List,
}

#[derive(Subcommand)]
enum WindowsAction {
    /// List capturable windows.
    List {
        /// Only windows owned by this process id.
        #[arg(long)]
        pid: Option<u32>,
        /// Only windows whose app name contains this (case-insensitive).
        #[arg(long)]
        app: Option<String>,
        /// Only windows whose title contains this (case-insensitive).
        #[arg(long)]
        title: Option<String>,
    },
}

#[derive(Subcommand)]
enum CamerasAction {
    /// List cameras.
    List {
        /// Probe each camera and report its live status.
        #[arg(long)]
        validate: bool,
    },
}

/// Combined `devices list` payload.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DevicesPayload {
    displays: Vec<DisplayInfo>,
    windows: Vec<WindowInfo>,
    microphones: Vec<AudioDeviceInfo>,
    cameras: Vec<CameraDeviceInfo>,
}

/// `doctor` payload: capabilities plus whether every listed capability is supported.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DoctorPayload {
    ready: bool,
    capabilities: CaptureCapabilities,
}

fn dispatch(cli: &Cli) -> Result<(), String> {
    match &cli.command {
        Command::Devices { .. } => {
            let payload = DevicesPayload {
                displays: displays(cli.thumbnails)?,
                windows: windows(cli.thumbnails, None, None, None)?,
                microphones: mics()?,
                cameras: cameras()?,
            };
            emit(&payload, cli.format)
        }
        Command::Displays { .. } => emit(&displays(cli.thumbnails)?, cli.format),
        Command::Windows {
            action: WindowsAction::List { pid, app, title },
        } => emit(
            &windows(cli.thumbnails, *pid, app.as_deref(), title.as_deref())?,
            cli.format,
        ),
        Command::Mics { .. } => emit(&mics()?, cli.format),
        Command::Cameras {
            action: CamerasAction::List { validate },
        } => {
            if *validate {
                emit(&validated_cameras()?, cli.format)
            } else {
                emit(&cameras()?, cli.format)
            }
        }
        Command::Capabilities => emit(&capabilities()?, cli.format),
        Command::Doctor => {
            let caps = capabilities()?;
            let ready = caps.capabilities.iter().all(|c| c.supported);
            emit(
                &DoctorPayload {
                    ready,
                    capabilities: caps,
                },
                cli.format,
            )
        }
        Command::Install => {
            let status = crate::path_install::status();
            let message = crate::path_install::install()?;
            emit(
                &json!({ "message": message, "binDir": status.bin_dir }),
                cli.format,
            )
        }
        Command::Uninstall => {
            let message = crate::path_install::uninstall()?;
            emit(&json!({ "message": message }), cli.format)
        }
        Command::Status => control(cli, "status", Value::Null),
        Command::Rec { action } => match action {
            RecAction::Start(args) => control(cli, "rec.start", build_start_params(args)?),
            RecAction::Stop => control(cli, "rec.stop", Value::Null),
            RecAction::Pause => control(cli, "rec.pause", Value::Null),
            RecAction::Resume => control(cli, "rec.resume", Value::Null),
            RecAction::Status => control(cli, "rec.status", Value::Null),
        },
        Command::Select { action } => control(cli, "intent.patch", select_patch(action)?),
        Command::Set { action } => control(cli, "intent.patch", set_patch(action)?),
        Command::Selection { action } => match action {
            SelectionAction::Show => control(cli, "intent.get", Value::Null),
            SelectionAction::Reset => control(cli, "intent.reset", Value::Null),
        },
    }
}

/// Build an intent patch from a `select` verb.
fn select_patch(action: &SelectAction) -> Result<Value, String> {
    Ok(match action {
        SelectAction::Screen { id } => {
            json!({ "targetType": "display", "targetId": id, "region": null })
        }
        SelectAction::Window { id } => {
            json!({ "targetType": "window", "targetId": id, "region": null })
        }
        SelectAction::Region { spec } => {
            json!({ "targetType": "region", "targetId": 0, "region": parse_region(spec)? })
        }
        SelectAction::Mic { value } => match value.as_str() {
            "none" => json!({ "microphone": false, "microphoneDeviceId": null }),
            "default" => json!({ "microphone": true, "microphoneDeviceId": null }),
            id => json!({ "microphone": true, "microphoneDeviceId": id }),
        },
        SelectAction::Camera { value } => match value.as_str() {
            "none" => json!({ "camera": false, "cameraDeviceId": null }),
            id => json!({ "camera": true, "cameraDeviceId": id }),
        },
    })
}

/// Build an intent patch from a `set` verb.
fn set_patch(action: &SetAction) -> Result<Value, String> {
    Ok(match action {
        SetAction::SystemAudio { value } => json!({ "systemAudio": parse_on_off(value)? }),
        SetAction::Fps { value } => json!({ "fps": value }),
        SetAction::Quality { value } => json!({ "quality": value }),
        SetAction::Countdown { value } => {
            let seconds = if value.eq_ignore_ascii_case("off") {
                Value::Null
            } else {
                json!(value
                    .parse::<u32>()
                    .map_err(|_| "countdown must be a number of seconds or `off`".to_string())?)
            };
            json!({ "countdown": seconds })
        }
    })
}

/// Send a control request to the running app and print its result.
fn control(cli: &Cli, method: &str, params: Value) -> Result<(), String> {
    let value = crate::control::send(method, params, !cli.no_launch, cli.timeout_ms)?;
    emit(&value, cli.format)
}

/// Build `rec.start` params. With a target flag it is a full one-off; with no
/// target flag it is empty, so the server records the stored capture intent.
fn build_start_params(args: &StartArgs) -> Result<Value, String> {
    let mut params = serde_json::Map::new();
    if let Some(id) = args.screen {
        params.insert("targetType".into(), json!("display"));
        params.insert("targetId".into(), json!(id));
    } else if let Some(id) = args.window {
        params.insert("targetType".into(), json!("window"));
        params.insert("targetId".into(), json!(id));
    } else if let Some(spec) = &args.region {
        params.insert("targetType".into(), json!("region"));
        params.insert("targetId".into(), json!(0));
        params.insert("region".into(), parse_region(spec)?);
    }

    // Option flags only apply to the explicit path; the stored intent carries
    // its own. Without a target, the flags are ignored (the intent wins).
    if !params.contains_key("targetType") {
        return Ok(Value::Object(params));
    }

    let mut options = serde_json::Map::new();
    if let Some(sa) = &args.system_audio {
        options.insert("systemAudio".into(), Value::Bool(parse_on_off(sa)?));
    }
    match args.mic.as_deref() {
        None | Some("none") => {}
        Some("default") => {
            options.insert("microphone".into(), Value::Bool(true));
        }
        Some(id) => {
            options.insert("microphone".into(), Value::Bool(true));
            options.insert("microphoneDeviceId".into(), Value::String(id.to_string()));
        }
    }
    match args.camera.as_deref() {
        None | Some("none") => {}
        Some(id) => {
            options.insert("camera".into(), Value::Bool(true));
            options.insert("cameraDeviceId".into(), Value::String(id.to_string()));
        }
    }
    if let Some(fps) = args.fps {
        options.insert("fps".into(), json!(fps));
    }
    if let Some(q) = &args.quality {
        options.insert("quality".into(), Value::String(q.clone()));
    }
    if !options.is_empty() {
        params.insert("options".into(), Value::Object(options));
    }
    Ok(Value::Object(params))
}

fn parse_region(spec: &str) -> Result<Value, String> {
    let n: Vec<i64> = spec
        .split(',')
        .map(|s| s.trim().parse::<i64>())
        .collect::<Result<_, _>>()
        .map_err(|_| "region must be four integers: X,Y,W,H".to_string())?;
    if n.len() != 4 {
        return Err("region must be four integers: X,Y,W,H".into());
    }
    Ok(json!({ "x": n[0], "y": n[1], "width": n[2], "height": n[3] }))
}

fn parse_on_off(v: &str) -> Result<bool, String> {
    match v.to_lowercase().as_str() {
        "on" | "true" | "1" | "yes" => Ok(true),
        "off" | "false" | "0" | "no" => Ok(false),
        other => Err(format!("expected on/off, got {other}")),
    }
}

fn displays(thumbnails: bool) -> Result<Vec<DisplayInfo>, String> {
    let list = block_on(crate::commands::get_displays())?;
    Ok(list
        .into_iter()
        .map(|mut d| {
            if !thumbnails {
                d.thumbnail = None;
            }
            d
        })
        .collect())
}

fn windows(
    thumbnails: bool,
    pid: Option<u32>,
    app: Option<&str>,
    title: Option<&str>,
) -> Result<Vec<WindowInfo>, String> {
    let list = block_on(crate::commands::get_windows())?;
    Ok(list
        .into_iter()
        .filter(|w| pid.is_none_or(|p| w.pid == p))
        .filter(|w| app.is_none_or(|a| w.app_name.to_lowercase().contains(&a.to_lowercase())))
        .filter(|w| title.is_none_or(|t| w.title.to_lowercase().contains(&t.to_lowercase())))
        .map(|mut w| {
            if !thumbnails {
                w.thumbnail = None;
            }
            w
        })
        .collect())
}

fn mics() -> Result<Vec<AudioDeviceInfo>, String> {
    block_on(crate::commands::get_audio_devices())
}

fn cameras() -> Result<Vec<CameraDeviceInfo>, String> {
    block_on(crate::commands::get_camera_devices())
}

fn validated_cameras() -> Result<Vec<CameraValidationResult>, String> {
    let devices = cameras()?;
    devices
        .into_iter()
        .map(|c| block_on(crate::commands::validate_camera_source(c.id)))
        .collect()
}

fn capabilities() -> Result<CaptureCapabilities, String> {
    block_on(crate::commands::capture_capabilities())
}

/// Drive an async command function to completion on Tauri's runtime. The
/// enumeration functions offload to `spawn_blocking` on that same runtime, so
/// this composes without a second runtime.
fn block_on<T, E: std::fmt::Display>(
    fut: impl std::future::Future<Output = Result<T, E>>,
) -> Result<T, String> {
    tauri::async_runtime::block_on(fut).map_err(|e| e.to_string())
}

/// Print a value in the chosen (or auto-detected) format. Data is JSON-modelled
/// throughout; YAML is a render-time convenience for human eyes.
fn emit<T: Serialize>(value: &T, format: Option<Format>) -> Result<(), String> {
    let format = format.unwrap_or_else(|| {
        // A terminal reader wants YAML; a pipe or `$x = recast ...` capture
        // wants machine-parseable JSON. `is_terminal` reflects the rebound
        // console handle on Windows too (see attach_parent_console).
        if std::io::stdout().is_terminal() {
            Format::Yaml
        } else {
            Format::Json
        }
    });
    match format {
        // serde_yaml already ends its output with a newline.
        Format::Yaml => {
            let yaml = serde_yaml::to_string(value).map_err(|e| e.to_string())?;
            print!("{yaml}");
        }
        Format::Json => {
            let json = serde_json::to_string(value).map_err(|e| e.to_string())?;
            println!("{json}");
        }
    }
    Ok(())
}

/// Make stdout/stderr usable for the release GUI-subsystem exe, which has no
/// console of its own.
///
/// If the caller already gave us a stdout (a console, or a pipe/file from
/// `recast status > out.json` or `$j = recast status`), leave it: that is where
/// the output must go, and clobbering it would break scripted capture. Only when
/// there is no usable stdout (interactive shell launching the GUI-subsystem
/// build) do we attach to the parent console and bind CONOUT$.
#[cfg(windows)]
fn attach_parent_console() {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        OPEN_EXISTING,
    };
    use windows::Win32::System::Console::{
        AttachConsole, GetStdHandle, SetStdHandle, ATTACH_PARENT_PROCESS, STD_ERROR_HANDLE,
        STD_OUTPUT_HANDLE,
    };

    unsafe {
        // A valid inherited stdout (redirect or existing console) is
        // authoritative: keep it so output reaches the caller's pipe/file.
        if let Ok(handle) = GetStdHandle(STD_OUTPUT_HANDLE) {
            if !handle.is_invalid() {
                return;
            }
        }
        if AttachConsole(ATTACH_PARENT_PROCESS).is_err() {
            return;
        }
        let conout: Vec<u16> = "CONOUT$\0".encode_utf16().collect();
        if let Ok(handle) = CreateFileW(
            PCWSTR(conout.as_ptr()),
            (FILE_GENERIC_READ | FILE_GENERIC_WRITE).0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            Default::default(),
            None,
        ) {
            let _ = SetStdHandle(STD_OUTPUT_HANDLE, handle);
            let _ = SetStdHandle(STD_ERROR_HANDLE, handle);
        }
    }
}
