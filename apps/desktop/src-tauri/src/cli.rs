//! Headless CLI branch. Parsed in `main` before the Tauri app boots so agents
//! and scripts can drive Recast without a running window.
//!
//! Enumeration verbs reuse the same functions the UI invokes; control verbs
//! (status, rec) talk to a running app over the local socket (see control.rs).
//! Output is YAML at a terminal and JSON when piped or captured, overridable
//! with `--format`. See docs/cli-automation-plan.md.

use std::io::IsTerminal;
use std::path::PathBuf;

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
    "profile",
    "screenshot",
    "screen-read",
    "transcribe",
    "watch",
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
    /// List, inspect, or apply saved recording profiles.
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },
    /// Capture a PNG of a display, a window, or Recast's own UI. Lets an agent
    /// see on-screen state and decide when a step is done or what to do next.
    Screenshot {
        #[command(subcommand)]
        target: ScreenshotTarget,
    },
    /// Read a video file into a timestamped, structured text timeline (OCR). Lets
    /// an agent understand what happened in a recording without narration. Needs
    /// the app's `ocr` feature; routes through the running instance.
    ScreenRead {
        /// Path to the video file (e.g. an .mp4) to read.
        input: String,
    },
    /// Transcribe an audio file against a downloaded `.gguf` model. Offline —
    /// does not need the app or the GUI to be running. The CLI path into the
    /// on-device engine; also used by the CI / release smoke test.
    ///
    /// SMOKE_TEST_VERB: the script `scripts/release/smoke-test-transcription.ps1`
    /// calls this verb with these exact flag names. Rename the verb or change
    /// any flag (`--input`, `--model`, `--out`, `--language`) and update the
    /// script's `$TranscribeVerb` in the same commit. CI smoke tests will
    /// start failing otherwise — that is by design, not noise.
    Transcribe(TranscribeArgs),
    /// Stream backend events (recording + selection + profiles) until interrupted.
    Watch {
        /// Comma-separated event groups: `rec`, `selection`, `profiles` (default: all).
        #[arg(long)]
        events: Option<String>,
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

#[derive(Subcommand)]
enum ProfileAction {
    /// List all saved profiles.
    List,
    /// Apply a profile (by id or name) to the staged capture intent.
    Use {
        #[arg(value_name = "ID|NAME")]
        id: String,
    },
    /// Print one profile (by id or name).
    Show {
        #[arg(value_name = "ID|NAME")]
        id: String,
    },
}

#[derive(Subcommand)]
enum ScreenshotTarget {
    /// A whole display by id (from `displays list`).
    Display {
        id: u32,
        #[command(flatten)]
        shot: ShotArgs,
    },
    /// One application window by id (from `windows list`).
    Window {
        id: u32,
        #[command(flatten)]
        shot: ShotArgs,
    },
    /// Recast's own UI: the focused window, or a specific one by label.
    App {
        /// Window label to capture (default: the focused Recast window).
        #[arg(long)]
        window: Option<String>,
        #[command(flatten)]
        shot: ShotArgs,
    },
}

/// Output options shared by every screenshot target.
#[derive(clap::Args)]
struct ShotArgs {
    /// Where to write the PNG. Defaults to a timestamped file in the temp dir.
    #[arg(long, value_name = "PATH")]
    out: Option<std::path::PathBuf>,
    /// Cap the longest edge to this many pixels (keeps agent shots small).
    #[arg(long, value_name = "PX", default_value_t = 1600)]
    max: u32,
    /// Capture at native resolution (ignore --max).
    #[arg(long)]
    full: bool,
    /// Also print a base64 data URI of the image alongside the file path.
    #[arg(long)]
    base64: bool,
}

/// CLI args for the `transcribe` verb. Mirrors the flag shape the smoke test
/// script (`scripts/release/smoke-test-transcription.ps1`) expects — keep
/// both in lockstep.
#[derive(clap::Args)]
struct TranscribeArgs {
    /// Audio file to transcribe (any format FFmpeg can read).
    #[arg(long, value_name = "PATH")]
    input: PathBuf,
    /// Path to a downloaded `.gguf` model file. A single GGUF — the format
    /// the on-device engine loads directly. The smoke test uses
    /// `whisper-base-Q5_K_M.gguf` from `models-cache/smoke-test/`.
    #[arg(long, value_name = "PATH")]
    model: PathBuf,
    /// ISO source-language hint (e.g. `en`). Omit to let the model autodetect.
    #[arg(long)]
    language: Option<String>,
    /// Where to write the transcript as JSON. Defaults to stdout when omitted.
    #[arg(long, value_name = "PATH")]
    out: Option<PathBuf>,
}

impl ShotArgs {
    /// Longest-edge cap to pass down: 0 (native) when `--full`, else `--max`.
    fn max_edge(&self) -> u32 {
        if self.full {
            0
        } else {
            self.max
        }
    }

    /// Absolute output path, if one was given. Resolved against the CLI's CWD so
    /// the `app` shot (written by the app process) lands where the agent expects.
    fn resolved_out(&self) -> Option<std::path::PathBuf> {
        self.out
            .clone()
            .map(crate::commands::screenshot::absolutize)
    }
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
    /// Auto-stop after this long, e.g. `30s`, `5m`, or a plain number of seconds.
    /// Backend-owned, so it fires even after the CLI process exits.
    #[arg(long, value_name = "DURATION")]
    timeout: Option<String>,
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
        Command::Profile { action } => match action {
            ProfileAction::List => control(cli, "profile.list", Value::Null),
            ProfileAction::Use { id } => control(cli, "profile.use", json!({ "id": id })),
            ProfileAction::Show { id } => show_profile(cli, id),
        },
        Command::Screenshot { target } => screenshot(cli, target),
        Command::ScreenRead { input } => {
            let abs = crate::commands::screenshot::absolutize(std::path::PathBuf::from(input));
            control(cli, "screen.read", json!({ "path": abs.to_string_lossy() }))
        }
        Command::Transcribe(args) => {
            // The model_id we hand to the Transcript is informational only —
            // the engine doesn't read it. Default to the file stem so the
            // smoke test output identifies which GGUF was used.
            let model_id = args
                .model
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("transcribe-cli")
                .to_string();
            let transcript = block_on(crate::transcription::transcribe_for_paths(
                &args.input,
                &args.model,
                &model_id,
                args.language.as_deref(),
            ))?;
            let json = serde_json::to_string_pretty(&transcript).map_err(|e| e.to_string())?;
            match &args.out {
                Some(path) => std::fs::write(path, format!("{json}\n"))
                    .map_err(|e| format!("write transcript: {e}"))?,
                None => println!("{json}"),
            }
            Ok(())
        }
        Command::Watch { events } => {
            let params = match events {
                Some(list) => {
                    let groups: Vec<&str> = list
                        .split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .collect();
                    json!({ "events": groups })
                }
                None => Value::Null,
            };
            crate::control::watch(params, !cli.no_launch, cli.timeout_ms, |frame| {
                let _ = emit(frame, cli.format);
            })
        }
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

/// Fetch the profile list and print the single entry matching `id` (by id, or
/// case-insensitive name). Filtering client-side keeps the control surface to
/// one `profile.list` method.
fn show_profile(cli: &Cli, id: &str) -> Result<(), String> {
    let snapshot =
        crate::control::send("profile.list", Value::Null, !cli.no_launch, cli.timeout_ms)?;
    let matched = snapshot
        .get("profiles")
        .and_then(Value::as_array)
        .and_then(|list| {
            list.iter().find(|p| {
                p.get("id").and_then(Value::as_str) == Some(id)
                    || p.get("name")
                        .and_then(Value::as_str)
                        .is_some_and(|n| n.eq_ignore_ascii_case(id))
            })
        });
    match matched {
        Some(profile) => emit(profile, cli.format),
        None => Err(format!("no profile matching '{id}'")),
    }
}

/// Capture a screenshot. Display/window shots run headlessly here (like the
/// enumeration verbs); the `app` shot goes through the running instance so it
/// can target its own focused window.
fn screenshot(cli: &Cli, target: &ScreenshotTarget) -> Result<(), String> {
    use crate::commands::screenshot::{capture_display, capture_window, ShotOptions};
    match target {
        ScreenshotTarget::Display { id, shot } => {
            let opts = ShotOptions {
                out: shot.resolved_out(),
                max_edge: shot.max_edge(),
                base64: shot.base64,
            };
            emit(&capture_display(*id, &opts)?, cli.format)
        }
        ScreenshotTarget::Window { id, shot } => {
            let opts = ShotOptions {
                out: shot.resolved_out(),
                max_edge: shot.max_edge(),
                base64: shot.base64,
            };
            emit(&capture_window(*id, &opts)?, cli.format)
        }
        ScreenshotTarget::App { window, shot } => {
            let mut params = serde_json::Map::new();
            if let Some(w) = window {
                params.insert("window".into(), json!(w));
            }
            if let Some(out) = shot.resolved_out() {
                params.insert("out".into(), json!(out.to_string_lossy()));
            }
            params.insert("maxEdge".into(), json!(shot.max_edge()));
            params.insert("base64".into(), json!(shot.base64));
            control(cli, "app.screenshot", Value::Object(params))
        }
    }
}

/// Build `rec.start` params. With a target flag it is a full one-off; with no
/// target flag it is empty, so the server records the stored capture intent.
fn build_start_params(args: &StartArgs) -> Result<Value, String> {
    let mut params = serde_json::Map::new();
    // Auto-stop applies to both the explicit and intent-based start paths.
    if let Some(t) = &args.timeout {
        params.insert("timeoutMs".into(), json!(parse_duration_ms(t)?));
    }
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

/// Parse `30s` / `5m` / `2h` / `500ms` / a plain number of seconds into ms.
fn parse_duration_ms(spec: &str) -> Result<u64, String> {
    let spec = spec.trim();
    // Check `ms` before `s` since both end in `s`.
    let (num, mult) = if let Some(n) = spec.strip_suffix("ms") {
        (n, 1u64)
    } else if let Some(n) = spec.strip_suffix('s') {
        (n, 1_000)
    } else if let Some(n) = spec.strip_suffix('m') {
        (n, 60_000)
    } else if let Some(n) = spec.strip_suffix('h') {
        (n, 3_600_000)
    } else {
        (spec, 1_000)
    };
    let value: u64 = num
        .trim()
        .parse()
        .map_err(|_| format!("invalid duration: {spec}"))?;
    Ok(value * mult)
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn duration_parsing() {
        assert_eq!(parse_duration_ms("30").unwrap(), 30_000);
        assert_eq!(parse_duration_ms("30s").unwrap(), 30_000);
        assert_eq!(parse_duration_ms("5m").unwrap(), 300_000);
        assert_eq!(parse_duration_ms("2h").unwrap(), 7_200_000);
        assert_eq!(parse_duration_ms("500ms").unwrap(), 500);
        assert_eq!(parse_duration_ms(" 10s ").unwrap(), 10_000);
        assert!(parse_duration_ms("abc").is_err());
        assert!(parse_duration_ms("").is_err());
    }

    #[test]
    fn region_parsing() {
        assert_eq!(
            parse_region("1,2,3,4").unwrap(),
            json!({"x":1,"y":2,"width":3,"height":4})
        );
        assert_eq!(
            parse_region(" 10, 20, 30, 40 ").unwrap(),
            json!({"x":10,"y":20,"width":30,"height":40})
        );
        assert!(parse_region("1,2,3").is_err());
        assert!(parse_region("a,b,c,d").is_err());
    }

    #[test]
    fn on_off_parsing() {
        assert!(parse_on_off("on").unwrap());
        assert!(parse_on_off("ON").unwrap());
        assert!(!parse_on_off("off").unwrap());
        assert!(parse_on_off("bad").is_err());
    }

    #[test]
    fn select_patch_builds_targets_and_devices() {
        assert_eq!(
            select_patch(&SelectAction::Screen { id: 7 }).unwrap(),
            json!({"targetType":"display","targetId":7,"region":null})
        );
        assert_eq!(
            select_patch(&SelectAction::Window { id: 9 }).unwrap(),
            json!({"targetType":"window","targetId":9,"region":null})
        );
        assert_eq!(
            select_patch(&SelectAction::Mic {
                value: "none".into()
            })
            .unwrap(),
            json!({"microphone":false,"microphoneDeviceId":null})
        );
        assert_eq!(
            select_patch(&SelectAction::Mic {
                value: "default".into()
            })
            .unwrap(),
            json!({"microphone":true,"microphoneDeviceId":null})
        );
        assert_eq!(
            select_patch(&SelectAction::Mic {
                value: "usb-1".into()
            })
            .unwrap(),
            json!({"microphone":true,"microphoneDeviceId":"usb-1"})
        );
        assert_eq!(
            select_patch(&SelectAction::Camera {
                value: "none".into()
            })
            .unwrap(),
            json!({"camera":false,"cameraDeviceId":null})
        );
        assert_eq!(
            select_patch(&SelectAction::Camera {
                value: "Webcam".into()
            })
            .unwrap(),
            json!({"camera":true,"cameraDeviceId":"Webcam"})
        );
    }

    #[test]
    fn set_patch_builds_options() {
        assert_eq!(
            set_patch(&SetAction::SystemAudio {
                value: "off".into()
            })
            .unwrap(),
            json!({"systemAudio":false})
        );
        assert_eq!(
            set_patch(&SetAction::Fps { value: 60 }).unwrap(),
            json!({"fps":60})
        );
        assert_eq!(
            set_patch(&SetAction::Quality {
                value: "high".into()
            })
            .unwrap(),
            json!({"quality":"high"})
        );
        assert_eq!(
            set_patch(&SetAction::Countdown {
                value: "off".into()
            })
            .unwrap(),
            json!({"countdown":null})
        );
        assert_eq!(
            set_patch(&SetAction::Countdown { value: "3".into() }).unwrap(),
            json!({"countdown":3})
        );
        assert!(set_patch(&SetAction::Countdown { value: "x".into() }).is_err());
    }

    fn blank_start_args() -> StartArgs {
        StartArgs {
            screen: None,
            window: None,
            region: None,
            mic: None,
            camera: None,
            system_audio: None,
            fps: None,
            quality: None,
            timeout: None,
        }
    }

    #[test]
    fn start_params_no_target_is_empty() {
        // No target flag => empty params, so the server records the stored intent.
        assert_eq!(build_start_params(&blank_start_args()).unwrap(), json!({}));
    }

    #[test]
    fn start_params_timeout_without_target() {
        let mut a = blank_start_args();
        a.timeout = Some("30s".into());
        assert_eq!(build_start_params(&a).unwrap(), json!({"timeoutMs":30_000}));
    }

    #[test]
    fn start_params_explicit_target_with_options() {
        let mut a = blank_start_args();
        a.screen = Some(5);
        a.mic = Some("default".into());
        a.system_audio = Some("off".into());
        let p = build_start_params(&a).unwrap();
        assert_eq!(p["targetType"], json!("display"));
        assert_eq!(p["targetId"], json!(5));
        assert_eq!(p["options"]["microphone"], json!(true));
        assert_eq!(p["options"]["systemAudio"], json!(false));
    }
}
