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
    "project",
    "editor",
    "export",
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
    /// Read a project's editor state (`edits.json`) and derived timeline.
    /// Phase A — read-only; no mutations, no lock acquisition.
    Project {
        #[command(subcommand)]
        action: ProjectAction,
    },
    /// Targeted edits to a project (trim, cuts, ...). All subcommands acquire
    /// the project write-lock, validate, and persist via `save_project_edits`.
    Editor {
        #[command(subcommand)]
        action: EditorAction,
    },
    /// Read or queue the export job queue. `start` enqueues; `wait` blocks
    /// until a job reaches a terminal state; `cancel` aborts.
    Export {
        #[command(subcommand)]
        action: ExportAction,
    },
    /// Propose edits without touching the project. Ops are journalled against
    /// the state they forked from; a human reviews the diff and applies.
    Branch {
        #[command(subcommand)]
        action: BranchAction,
    },
}

#[derive(Subcommand)]
enum BranchAction {
    /// Fork a branch from the project's current state.
    Create {
        path: String,
        #[arg(long, value_name = "ID")]
        branch: String,
        /// Who is proposing, e.g. `agent:claude`.
        #[arg(long, value_name = "ID")]
        author: String,
        #[arg(long, value_name = "TEXT")]
        label: Option<String>,
    },
    /// Open branches for a project.
    List { path: String },
    /// Record ops onto a branch as one atomic entry.
    Append {
        path: String,
        #[arg(long, value_name = "ID")]
        branch: String,
        /// Retry-safe key. Re-sending one already on the branch is a no-op.
        #[arg(long, value_name = "KEY")]
        idem_key: String,
        /// JSON array of ops, e.g. `[{"op":"cutAdd","start":1,"end":2}]`.
        #[arg(long, value_name = "JSON")]
        ops: Option<String>,
        /// Read the ops array from stdin instead of `--ops`.
        #[arg(long)]
        from_stdin: bool,
        /// Reject unless the branch is at this sequence number.
        #[arg(long, value_name = "N")]
        expect_seq: Option<u64>,
    },
    /// Field-level changes the branch would make.
    Diff {
        path: String,
        #[arg(long, value_name = "ID")]
        branch: String,
    },
    /// The full render state the branch would produce.
    Show {
        path: String,
        #[arg(long, value_name = "ID")]
        branch: String,
    },
    /// Drop every entry after a sequence number.
    Truncate {
        path: String,
        #[arg(long, value_name = "ID")]
        branch: String,
        #[arg(long, value_name = "N")]
        seq: u64,
    },
    /// Delete a branch without applying it.
    Discard {
        path: String,
        #[arg(long, value_name = "ID")]
        branch: String,
    },
    /// Write the branch into the project. Takes the write-lock and is
    /// fast-forward only: a project edited since the fork is rejected.
    Apply {
        path: String,
        #[arg(long, value_name = "ID")]
        branch: String,
        #[arg(long, value_name = "ID")]
        writer_id: String,
    },
}

#[derive(Subcommand)]
enum ProjectAction {
    /// Open a `.recast` (or bare video) and print the full editor document.
    Open {
        /// Path to the `.recast` archive or source video.
        path: String,
    },
    /// Print the project's current `edits.json` (the same `RenderState` the editor ships).
    Show { path: String },
    /// Derive the project's kept-segment timeline (trim, cuts, output duration).
    Timeline { path: String },
    /// List the project's zoom regions.
    ZoomRegions { path: String },
    /// List the project's annotations.
    Annotations { path: String },
    /// Acquire the project write-lock. Subsequent mutate/exports by either
    /// side block until release. Idempotent for the same caller.
    Lock {
        path: String,
        /// Holder kind (default: `agent`).
        #[arg(long, value_name = "ui|agent", default_value = "agent")]
        r#as: String,
        /// Stable id (`agent:<id>` for the CLI; the GUI uses `ui:<user>`).
        #[arg(long, value_name = "ID")]
        writer_id: String,
    },
    /// Release the write-lock. By default only the holder can release; pass
    /// `--force` to evict a stale or wrong-owner lock (use with care — it
    /// erases the GUI's silent write window).
    Unlock {
        /// Release even if the lock is held by another id.
        #[arg(long)]
        force: bool,
        /// The agent's writer id (matches the `lock --writer-id`).
        #[arg(long, value_name = "ID")]
        writer_id: String,
    },
    /// Replace the project's `edits.json` with a full RenderState JSON
    /// from a file or stdin. Validates then writes via `save_project_edits`.
    Patch {
        path: String,
        #[arg(long, value_name = "PATH")]
        from_file: Option<String>,
        /// Read the JSON from stdin instead of `--from-file`.
        #[arg(long)]
        from_stdin: bool,
        #[arg(long, value_name = "ID")]
        writer_id: String,
    },
}

#[derive(Subcommand)]
enum EditorAction {
    /// Set the trim window (trim-start, trim-end). Validates then saves.
    Trim {
        path: String,
        #[arg(long, value_name = "SECONDS")]
        start: f64,
        #[arg(long, value_name = "SECONDS")]
        end: f64,
        #[arg(long, value_name = "ID")]
        writer_id: String,
    },
    /// Universal mutator. Set any scalar/struct field in RenderState by
    /// dotted-path JSON pointer; e.g. `borderRadius`, `cursorSize`,
    /// `audioSettings.volume`, `cursorSettings.size`. Pair with
    /// `--value <JSON>` (string for strings, number, true/false,
    /// array, object). For array fields where you want to add or
    /// remove entries use the targeted verbs (cut/zoom/split-point/
    /// speed/animations/annotations) instead.
    Set {
        path: String,
        /// Dotted JSON pointer inside `RenderState`, e.g.
        /// `borderRadius`, `cursorSize`, `audioSettings.volume`,
        /// `cursorSettings.size`, `annotations.0.fill`.
        #[arg(long, value_name = "DOTTED.PATH")]
        field: String,
        /// JSON value to set. Strings need quoting inside `--value`;
        /// numbers, true/false, arrays, and objects are also JSON.
        #[arg(long, value_name = "JSON")]
        value: String,
        #[arg(long, value_name = "ID")]
        writer_id: String,
    },
    /// Cut ranges on the timeline.
    Cut {
        #[command(subcommand)]
        action: CutAction,
    },
    /// Zoom regions on the timeline.
    Zoom {
        #[command(subcommand)]
        action: ZoomAction,
    },
    /// Split markers — original-time seconds that divide the kept clip
    /// into addressable segments.
    SplitPoint {
        #[command(subcommand)]
        action: SplitPointAction,
    },
    /// Per-segment speed overrides (empty = every segment plays at 1×).
    Speed {
        #[command(subcommand)]
        action: SpeedAction,
    },
    /// Per-segment scene animations — entrance/exit transforms on the
    /// video layer, anchored to a segment's original start time.
    Animations {
        #[command(subcommand)]
        action: AnimationsAction,
    },
    /// Annotations on the timeline — rect/ellipse/arrow/image/blur/text.
    Annotations {
        #[command(subcommand)]
        action: AnnotationsAction,
    },
}

#[derive(Subcommand)]
enum CutAction {
    Add {
        path: String,
        #[arg(long, value_name = "SECONDS")]
        start: f64,
        #[arg(long, value_name = "SECONDS")]
        end: f64,
        #[arg(long, value_name = "ID")]
        writer_id: String,
    },
    /// Remove a cut by --index or by matching `(start, end)`.
    Remove {
        path: String,
        #[arg(long, value_name = "INDEX")]
        index: Option<usize>,
        #[arg(long, value_name = "SECONDS", conflicts_with = "index")]
        start: Option<f64>,
        #[arg(long, value_name = "SECONDS")]
        end: Option<f64>,
        #[arg(long, value_name = "ID")]
        writer_id: String,
    },
    /// List every cut with its positional index.
    List { path: String },
}

#[derive(Subcommand)]
enum ZoomAction {
    Add {
        path: String,
        #[arg(long, value_name = "SECONDS")]
        start: f64,
        #[arg(long, value_name = "SECONDS")]
        end: f64,
        #[arg(long)]
        scale: f64,
        #[arg(long, default_value_t = 0.5, value_name = "UV")]
        center_x: f64,
        #[arg(long, default_value_t = 0.5, value_name = "UV")]
        center_y: f64,
        #[arg(long, default_value_t = 0.35, value_name = "SECONDS")]
        ramp_in: f64,
        #[arg(long, default_value_t = 0.35, value_name = "SECONDS")]
        ramp_out: f64,
        #[arg(long)]
        hidden: bool,
        #[arg(long, value_name = "ID")]
        writer_id: String,
    },
    /// Remove a zoom region by positional --index.
    Remove {
        path: String,
        #[arg(long, value_name = "INDEX")]
        index: usize,
        #[arg(long, value_name = "ID")]
        writer_id: String,
    },
    /// List every zoom region with its positional index.
    List { path: String },
}

#[derive(Subcommand)]
enum SplitPointAction {
    Add {
        path: String,
        #[arg(long, value_name = "SECONDS")]
        at: f64,
        #[arg(long, value_name = "ID")]
        writer_id: String,
    },
    Remove {
        path: String,
        #[arg(long, value_name = "SECONDS")]
        at: f64,
        #[arg(long, value_name = "ID")]
        writer_id: String,
    },
    List {
        path: String,
    },
}

#[derive(Subcommand)]
enum SpeedAction {
    /// Set or upsert a speed override at a segment's original start.
    Set {
        path: String,
        #[arg(long, value_name = "SECONDS")]
        segment_start: f64,
        #[arg(long, value_name = "RATE")]
        rate: f64,
        #[arg(long, value_name = "ID")]
        writer_id: String,
    },
    Remove {
        path: String,
        #[arg(long, value_name = "SECONDS")]
        segment_start: f64,
        #[arg(long, value_name = "ID")]
        writer_id: String,
    },
    List {
        path: String,
    },
}

#[derive(Subcommand)]
enum AnimationsAction {
    /// Add or upsert a scene animation at a segment's original start.
    /// Pass `--in "<spec JSON>"` and/or `--out "<spec JSON>"` (each a
    /// `SceneAnimSpec`: `{"kind":"fade|slide|scale|pop","durationMs":N,
    /// "easing":{...},"dir":"left|right|up|down","intensity":N}`).
    Add {
        path: String,
        #[arg(long, value_name = "SECONDS")]
        start: f64,
        #[arg(long, value_name = "JSON")]
        r#in: Option<String>,
        #[arg(long, value_name = "JSON")]
        out: Option<String>,
        #[arg(long, value_name = "ID")]
        writer_id: String,
    },
    Remove {
        path: String,
        #[arg(long, value_name = "SECONDS")]
        start: f64,
        #[arg(long, value_name = "ID")]
        writer_id: String,
    },
    List {
        path: String,
    },
}

#[derive(Subcommand)]
enum AnnotationsAction {
    /// Add an annotation. `--kind` selects the geometry type
    /// (`rect`|`ellipse`|`arrow`|`blur`|`image`|`text`). `--geometry` is
    /// a JSON object with the per-kind fields (e.g. for `rect`:
    /// `{"x":0.1,"y":0.1,"w":0.3,"h":0.2,"radius":0.02}`).
    Add {
        path: String,
        #[arg(long, value_name = "rect|ellipse|arrow|blur|image|text")]
        kind: String,
        #[arg(long, value_name = "JSON")]
        geometry: String,
        #[arg(long, value_name = "SECONDS")]
        start: f64,
        #[arg(long, value_name = "SECONDS")]
        end: f64,
        #[arg(long, default_value_t = 1.0, value_name = "0..1")]
        opacity: f64,
        #[arg(long, value_name = "LABEL")]
        name: Option<String>,
        /// Optional explicit id; a unique default is generated if omitted.
        #[arg(long, value_name = "ID")]
        id: Option<String>,
        #[arg(long, value_name = "ID")]
        writer_id: String,
    },
    /// Update an annotation by id. `--patch` is a partial JSON object
    /// merged over the existing row (top-level fields only: `start`,
    /// `end`, `opacity`, `hidden`, `name`, `z`, the entire `kind`, …).
    Update {
        path: String,
        #[arg(long, value_name = "ID")]
        id: String,
        #[arg(long, value_name = "JSON")]
        patch: String,
        #[arg(long, value_name = "ID")]
        writer_id: String,
    },
    Remove {
        path: String,
        #[arg(long, value_name = "ID")]
        id: String,
        #[arg(long, value_name = "ID")]
        writer_id: String,
    },
    List {
        path: String,
    },
}

#[derive(Subcommand)]
enum ExportAction {
    /// List every export job (status, phase, progress, output path).
    List,
    /// Show one export job by id.
    Show {
        /// The export job id (`recast export list` to find one).
        id: String,
    },
    /// Queue a new export of the given `.recast`. Validates the render state,
    /// then enqueues; the running job id is printed for `wait`/`cancel`.
    Start {
        path: String,
        #[arg(long, value_name = "mp4|webm|gif", default_value = "mp4")]
        format: String,
        #[arg(
            long,
            value_name = "auto|balanced|high|pristine",
            default_value = "balanced"
        )]
        quality: String,
        /// Encoder effort axis (orthogonal to `quality`): `fast` (quick),
        /// `balanced` (default), `quality` (slow, smaller file).
        #[arg(long, value_name = "fast|balanced|quality")]
        speed: Option<String>,
        /// Output frame rate; clamped to `<= source fps`. Ignored for GIF
        /// (use `--gif-fps` instead).
        #[arg(long, value_name = "FPS")]
        fps: Option<f64>,
        /// Burn the recorded captions into the video itself. Has no
        /// effect when no transcript exists in the render state.
        #[arg(long)]
        burn_captions: bool,
        /// Emit a sidecar subtitles file next to the export:
        /// `vtt` or `srt`. Implies a transcript must exist; empty
        /// transcripts yield an empty sidecar.
        #[arg(long, value_name = "vtt|srt")]
        caption_sidecar: Option<String>,
        /// Override the GIF frame rate (default = quality-profile gif_fps).
        #[arg(long, value_name = "FPS")]
        gif_fps: Option<u32>,
        /// GIF palette quality: `low` (64), `medium` (128), `high` (256).
        #[arg(long, value_name = "low|medium|high")]
        gif_quality: Option<String>,
        /// GIF loop count: `infinite`, `once`, or a non-negative integer.
        #[arg(long, value_name = "infinite|once|<n>")]
        gif_loop: Option<String>,
        /// GIF dither: `bayer` (default), `sierra2`, or `none`.
        #[arg(long, value_name = "bayer|sierra2|none")]
        gif_dither: Option<String>,
        /// Override the project write-lock's TTL for the duration of this
        /// export. Default 60s.
        #[arg(long, value_name = "ID")]
        writer_id: String,
    },
    /// Cancel a queued or running export by id.
    Cancel { id: String },
    /// Block until a job finishes (terminal status). Polls `export.show` so
    /// no streaming protocol is needed; default timeout is 10m.
    Wait {
        id: String,
        #[arg(long, value_name = "DURATION", default_value = "10m")]
        timeout: String,
        #[arg(long, value_name = "DURATION", default_value = "1s")]
        interval: String,
    },
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
        // Phase A (read-only): every verb hits `load_editor_document` /
        // `list_export_jobs` through the existing control channel. Each takes
        // a single `path`/`id` arg, so no JSON-patch surface here yet.
        Command::Project { action } => project_dispatch(cli, action),
        Command::Editor { action } => editor_dispatch(cli, action),
        Command::Export { action } => export_dispatch(cli, action),
        Command::Branch { action } => branch_dispatch(cli, action),
    }
}

/// Dispatch one `recast project ...` verb. The path is required for every
/// subcommand; v1 has no "active project" tracking, so the agent must pass it
/// explicitly. Phase B adds `lock`/`unlock`/`patch` on top of the read surface.
fn project_dispatch(cli: &Cli, action: &ProjectAction) -> Result<(), String> {
    match action {
        ProjectAction::Open { path }
        | ProjectAction::Show { path }
        | ProjectAction::Timeline { path }
        | ProjectAction::ZoomRegions { path }
        | ProjectAction::Annotations { path } => {
            let path = crate::commands::screenshot::absolutize(std::path::PathBuf::from(path))
                .to_string_lossy()
                .into_owned();
            let method = match action {
                ProjectAction::Open { .. } => "editor.open",
                ProjectAction::Show { .. } => "editor.show",
                ProjectAction::Timeline { .. } => "editor.timeline",
                ProjectAction::ZoomRegions { .. } => "editor.zoom-regions",
                ProjectAction::Annotations { .. } => "editor.annotations",
                _ => unreachable!(),
            };
            let value = crate::control::send(
                method,
                json!({ "path": path }),
                !cli.no_launch,
                cli.timeout_ms,
            )?;
            emit(&value, cli.format)
        }
        ProjectAction::Lock {
            path,
            r#as,
            writer_id,
        } => {
            let path = crate::commands::screenshot::absolutize(std::path::PathBuf::from(path))
                .to_string_lossy()
                .into_owned();
            let value = crate::control::send(
                "editor.lock",
                json!({ "path": path, "kind": r#as, "writerId": writer_id }),
                !cli.no_launch,
                cli.timeout_ms,
            )?;
            emit(&value, cli.format)
        }
        ProjectAction::Unlock { force, writer_id } => {
            let value = crate::control::send(
                "editor.unlock",
                json!({ "force": force, "writerId": writer_id }),
                !cli.no_launch,
                cli.timeout_ms,
            )?;
            emit(&value, cli.format)
        }
        ProjectAction::Patch {
            path,
            from_file,
            from_stdin,
            writer_id,
        } => {
            let path = crate::commands::screenshot::absolutize(std::path::PathBuf::from(path))
                .to_string_lossy()
                .into_owned();
            let render_state = read_render_state_json(from_file.as_deref(), *from_stdin)?;
            let value = crate::control::send(
                "editor.patch",
                json!({
                    "path": path,
                    "writerId": writer_id,
                    "renderState": render_state,
                }),
                !cli.no_launch,
                cli.timeout_ms,
            )?;
            emit(&value, cli.format)
        }
    }
}

/// Load a `RenderState` JSON from a file or stdin. Used by `project patch`.
fn read_render_state_json(from_file: Option<&str>, from_stdin: bool) -> Result<Value, String> {
    match (from_file, from_stdin) {
        (Some(_), true) => Err("--from-file and --from-stdin are mutually exclusive".into()),
        (Some(p), false) => {
            let bytes = std::fs::read(p).map_err(|e| format!("read {p}: {e}"))?;
            serde_json::from_slice(&bytes).map_err(|e| format!("parse {p}: {e}"))
        }
        (None, true) => {
            let mut s = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut s)
                .map_err(|e| format!("read stdin: {e}"))?;
            serde_json::from_str(&s).map_err(|e| format!("parse stdin: {e}"))
        }
        (None, false) => Err("project patch requires --from-file <path> or --from-stdin".into()),
    }
}

fn editor_dispatch(cli: &Cli, action: &EditorAction) -> Result<(), String> {
    match action {
        EditorAction::Trim {
            path,
            start,
            end,
            writer_id,
        } => {
            let path = crate::commands::screenshot::absolutize(std::path::PathBuf::from(path))
                .to_string_lossy()
                .into_owned();
            send_and_emit(
                cli,
                "editor.trim",
                json!({
                    "path": path,
                    "trimStart": start,
                    "trimEnd": end,
                    "writerId": writer_id,
                }),
            )
        }
        EditorAction::Set {
            path,
            field,
            value,
            writer_id,
        } => {
            let path = crate::commands::screenshot::absolutize(std::path::PathBuf::from(path))
                .to_string_lossy()
                .into_owned();
            let parsed: Value = serde_json::from_str(value)
                .map_err(|e| format!("--value is not valid JSON: {e}"))?;
            send_and_emit(
                cli,
                "editor.set",
                json!({
                    "path": path,
                    "field": field,
                    "value": parsed,
                    "writerId": writer_id,
                }),
            )
        }
        EditorAction::Cut { action } => cut_dispatch(cli, action),
        EditorAction::Zoom { action } => zoom_dispatch(cli, action),
        EditorAction::SplitPoint { action } => split_point_dispatch(cli, action),
        EditorAction::Speed { action } => speed_dispatch(cli, action),
        EditorAction::Animations { action } => animations_dispatch(cli, action),
        EditorAction::Annotations { action } => annotations_dispatch(cli, action),
    }
}

fn cut_dispatch(cli: &Cli, action: &CutAction) -> Result<(), String> {
    let path_for = |p: &str| {
        crate::commands::screenshot::absolutize(std::path::PathBuf::from(p))
            .to_string_lossy()
            .into_owned()
    };
    match action {
        CutAction::Add {
            path,
            start,
            end,
            writer_id,
        } => send_and_emit(
            cli,
            "editor.cut.add",
            json!({"path": path_for(path), "start": start, "end": end, "writerId": writer_id}),
        ),
        CutAction::List { path } => {
            let path = path_for(path);
            send_and_emit(cli, "editor.cut.list", json!({"path": path}))
        }
        CutAction::Remove {
            path,
            index,
            start,
            end,
            writer_id,
        } => {
            let path = path_for(path);
            let mut params = json!({"path": path, "writerId": writer_id});
            if let Some(i) = index {
                params["index"] = json!(i);
            }
            if let Some(s) = start {
                params["start"] = json!(s);
            }
            if let Some(e) = end {
                params["end"] = json!(e);
            }
            send_and_emit(cli, "editor.cut.remove", params)
        }
    }
}

fn zoom_dispatch(cli: &Cli, action: &ZoomAction) -> Result<(), String> {
    let path_for = |p: &str| {
        crate::commands::screenshot::absolutize(std::path::PathBuf::from(p))
            .to_string_lossy()
            .into_owned()
    };
    match action {
        ZoomAction::Add {
            path,
            start,
            end,
            scale,
            center_x,
            center_y,
            ramp_in,
            ramp_out,
            hidden,
            writer_id,
        } => send_and_emit(
            cli,
            "editor.zoom.add",
            json!({
                "path": path_for(path),
                "start": start,
                "end": end,
                "scale": scale,
                "centerX": center_x,
                "centerY": center_y,
                "rampIn": ramp_in,
                "rampOut": ramp_out,
                "hidden": hidden,
                "writerId": writer_id,
            }),
        ),
        ZoomAction::Remove {
            path,
            index,
            writer_id,
        } => send_and_emit(
            cli,
            "editor.zoom.remove",
            json!({"path": path_for(path), "index": index, "writerId": writer_id}),
        ),
        ZoomAction::List { path } => {
            let path = path_for(path);
            send_and_emit(cli, "editor.zoom.list", json!({"path": path}))
        }
    }
}

fn split_point_dispatch(cli: &Cli, action: &SplitPointAction) -> Result<(), String> {
    let path_for = |p: &str| {
        crate::commands::screenshot::absolutize(std::path::PathBuf::from(p))
            .to_string_lossy()
            .into_owned()
    };
    match action {
        SplitPointAction::Add {
            path,
            at,
            writer_id,
        } => send_and_emit(
            cli,
            "editor.split-point.add",
            json!({"path": path_for(path), "at": at, "writerId": writer_id}),
        ),
        SplitPointAction::Remove {
            path,
            at,
            writer_id,
        } => send_and_emit(
            cli,
            "editor.split-point.remove",
            json!({"path": path_for(path), "at": at, "writerId": writer_id}),
        ),
        SplitPointAction::List { path } => {
            let path = path_for(path);
            send_and_emit(cli, "editor.split-point.list", json!({"path": path}))
        }
    }
}

fn speed_dispatch(cli: &Cli, action: &SpeedAction) -> Result<(), String> {
    let path_for = |p: &str| {
        crate::commands::screenshot::absolutize(std::path::PathBuf::from(p))
            .to_string_lossy()
            .into_owned()
    };
    match action {
        SpeedAction::Set {
            path,
            segment_start,
            rate,
            writer_id,
        } => send_and_emit(
            cli,
            "editor.speed.set",
            json!({
                "path": path_for(path),
                "segmentStart": segment_start,
                "rate": rate,
                "writerId": writer_id,
            }),
        ),
        SpeedAction::Remove {
            path,
            segment_start,
            writer_id,
        } => send_and_emit(
            cli,
            "editor.speed.remove",
            json!({"path": path_for(path), "segmentStart": segment_start, "writerId": writer_id}),
        ),
        SpeedAction::List { path } => {
            let path = path_for(path);
            send_and_emit(cli, "editor.speed.list", json!({"path": path}))
        }
    }
}

fn animations_dispatch(cli: &Cli, action: &AnimationsAction) -> Result<(), String> {
    let path_for = |p: &str| {
        crate::commands::screenshot::absolutize(std::path::PathBuf::from(p))
            .to_string_lossy()
            .into_owned()
    };
    match action {
        AnimationsAction::Add {
            path,
            start,
            r#in,
            out,
            writer_id,
        } => {
            let path = path_for(path);
            let mut params = json!({
                "path": path,
                "start": start,
                "writerId": writer_id,
            });
            if let Some(s) = r#in {
                params["in"] =
                    serde_json::from_str(s).map_err(|e| format!("--in is not valid JSON: {e}"))?;
            }
            if let Some(s) = out {
                params["out"] =
                    serde_json::from_str(s).map_err(|e| format!("--out is not valid JSON: {e}"))?;
            }
            send_and_emit(cli, "editor.animations.add", params)
        }
        AnimationsAction::Remove {
            path,
            start,
            writer_id,
        } => send_and_emit(
            cli,
            "editor.animations.remove",
            json!({"path": path_for(path), "start": start, "writerId": writer_id}),
        ),
        AnimationsAction::List { path } => {
            let path = path_for(path);
            send_and_emit(cli, "editor.animations.list", json!({"path": path}))
        }
    }
}

fn annotations_dispatch(cli: &Cli, action: &AnnotationsAction) -> Result<(), String> {
    let path_for = |p: &str| {
        crate::commands::screenshot::absolutize(std::path::PathBuf::from(p))
            .to_string_lossy()
            .into_owned()
    };
    match action {
        AnnotationsAction::Add {
            path,
            kind,
            geometry,
            start,
            end,
            opacity,
            name,
            id,
            writer_id,
        } => {
            let path = path_for(path);
            let geometry_json: Value = serde_json::from_str(geometry)
                .map_err(|e| format!("--geometry is not valid JSON: {e}"))?;
            let mut params = json!({
                "path": path,
                "kind": kind,
                "geometry": geometry_json,
                "start": start,
                "end": end,
                "opacity": opacity,
                "writerId": writer_id,
            });
            if let Some(n) = name {
                params["name"] = json!(n);
            }
            if let Some(i) = id {
                params["id"] = json!(i);
            }
            send_and_emit(cli, "editor.annotations.add", params)
        }
        AnnotationsAction::Update {
            path,
            id,
            patch,
            writer_id,
        } => {
            let path = path_for(path);
            let patch_json: Value = serde_json::from_str(patch)
                .map_err(|e| format!("--patch is not valid JSON: {e}"))?;
            let params = json!({
                "path": path,
                "id": id,
                "patch": patch_json,
                "writerId": writer_id,
            });
            send_and_emit(cli, "editor.annotations.update", params)
        }
        AnnotationsAction::Remove {
            path,
            id,
            writer_id,
        } => send_and_emit(
            cli,
            "editor.annotations.remove",
            json!({"path": path_for(path), "id": id, "writerId": writer_id}),
        ),
        AnnotationsAction::List { path } => {
            let path = path_for(path);
            send_and_emit(cli, "editor.annotations.list", json!({"path": path}))
        }
    }
}

fn branch_dispatch(cli: &Cli, action: &BranchAction) -> Result<(), String> {
    let path_for = |p: &str| {
        crate::commands::screenshot::absolutize(std::path::PathBuf::from(p))
            .to_string_lossy()
            .into_owned()
    };
    match action {
        BranchAction::Create {
            path,
            branch,
            author,
            label,
        } => send_and_emit(
            cli,
            "branch.create",
            json!({"path": path_for(path), "branch": branch, "author": author, "label": label}),
        ),
        BranchAction::List { path } => {
            send_and_emit(cli, "branch.list", json!({ "path": path_for(path) }))
        }
        BranchAction::Append {
            path,
            branch,
            idem_key,
            ops,
            from_stdin,
            expect_seq,
        } => {
            let ops = read_ops_json(ops.as_deref(), *from_stdin)?;
            let mut params = json!({
                "path": path_for(path),
                "branch": branch,
                "idemKey": idem_key,
                "ops": ops,
            });
            if let Some(seq) = expect_seq {
                params["expectSeq"] = json!(seq);
            }
            send_and_emit(cli, "branch.append", params)
        }
        BranchAction::Diff { path, branch } => send_and_emit(
            cli,
            "branch.diff",
            json!({"path": path_for(path), "branch": branch}),
        ),
        BranchAction::Show { path, branch } => send_and_emit(
            cli,
            "branch.materialize",
            json!({"path": path_for(path), "branch": branch}),
        ),
        BranchAction::Truncate { path, branch, seq } => send_and_emit(
            cli,
            "branch.truncate",
            json!({"path": path_for(path), "branch": branch, "seq": seq}),
        ),
        BranchAction::Discard { path, branch } => send_and_emit(
            cli,
            "branch.discard",
            json!({"path": path_for(path), "branch": branch}),
        ),
        BranchAction::Apply {
            path,
            branch,
            writer_id,
        } => send_and_emit(
            cli,
            "branch.apply",
            json!({"path": path_for(path), "branch": branch, "writerId": writer_id}),
        ),
    }
}

/// Load the ops array for `branch append` from `--ops` or stdin.
fn read_ops_json(ops: Option<&str>, from_stdin: bool) -> Result<Value, String> {
    let text = match (ops, from_stdin) {
        (Some(_), true) => return Err("--ops and --from-stdin are mutually exclusive".into()),
        (Some(text), false) => text.to_string(),
        (None, true) => {
            let mut buffer = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut buffer)
                .map_err(|e| format!("read stdin: {e}"))?;
            buffer
        }
        (None, false) => return Err("branch append requires --ops <JSON> or --from-stdin".into()),
    };
    let parsed: Value = serde_json::from_str(&text).map_err(|e| format!("parse ops: {e}"))?;
    if parsed.is_array() {
        Ok(parsed)
    } else {
        Err("ops must be a JSON array".into())
    }
}

fn send_and_emit(cli: &Cli, method: &str, params: Value) -> Result<(), String> {
    let value = crate::control::send(method, params, !cli.no_launch, cli.timeout_ms)?;
    emit(&value, cli.format)
}

fn export_dispatch(cli: &Cli, action: &ExportAction) -> Result<(), String> {
    match action {
        ExportAction::List => {
            let value =
                crate::control::send("export.list", Value::Null, !cli.no_launch, cli.timeout_ms)?;
            emit(&value, cli.format)
        }
        ExportAction::Show { id } => {
            let value = crate::control::send(
                "export.show",
                json!({ "id": id }),
                !cli.no_launch,
                cli.timeout_ms,
            )?;
            emit(&value, cli.format)
        }
        ExportAction::Start {
            path,
            format,
            quality,
            speed,
            fps,
            burn_captions,
            caption_sidecar,
            gif_fps,
            gif_quality,
            gif_loop,
            gif_dither,
            writer_id,
        } => {
            let path = crate::commands::screenshot::absolutize(std::path::PathBuf::from(path))
                .to_string_lossy()
                .into_owned();
            let mut params = json!({
                "path": path,
                "format": format,
                "quality": quality,
                "burnCaptions": burn_captions,
                "writerId": writer_id,
            });
            if let Some(s) = speed {
                params["speed"] = json!(s);
            }
            if let Some(f) = fps {
                params["fps"] = json!(f);
            }
            if let Some(s) = caption_sidecar {
                params["captionSidecar"] = json!(s);
            }
            if format == "gif" {
                if let Some(f) = gif_fps {
                    params["gifFps"] = json!(f);
                }
                if let Some(q) = gif_quality {
                    params["gifQuality"] = json!(q);
                }
                if let Some(l) = gif_loop {
                    // Accept "infinite" / "once" / "5" / numeric.
                    let parsed: Value = if l == "infinite" || l == "once" {
                        json!(l)
                    } else {
                        serde_json::from_str(l).map_err(|e| {
                            format!(
                                "--gif-loop must be 'infinite' | 'once' | <number>; got '{l}': {e}"
                            )
                        })?
                    };
                    params["gifLoop"] = parsed;
                }
                if let Some(d) = gif_dither {
                    params["gifDither"] = json!(d);
                }
            }
            send_and_emit(cli, "export.start", params)
        }
        ExportAction::Cancel { id } => {
            let value = crate::control::send(
                "export.cancel",
                json!({ "id": id }),
                !cli.no_launch,
                cli.timeout_ms,
            )?;
            emit(&value, cli.format)
        }
        ExportAction::Wait {
            id,
            timeout,
            interval,
        } => export_wait(cli, id, timeout, interval),
    }
}

/// Block until a job reaches a terminal status. Polls `export.show` rather
/// than streaming the `export-state` event (no protocol change required; 1s
/// default interval is well below human-perceptible export progress).
fn export_wait(cli: &Cli, id: &str, timeout: &str, interval: &str) -> Result<(), String> {
    use std::time::{Duration, Instant};

    let timeout_ms = parse_duration_ms(timeout).map_err(|e| format!("--timeout: {e}"))?;
    let interval_ms = parse_duration_ms(interval).map_err(|e| format!("--interval: {e}"))?;
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        let value = crate::control::send(
            "export.show",
            json!({ "id": id }),
            !cli.no_launch,
            cli.timeout_ms,
        )?;
        let status = value
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if matches!(
            status.as_str(),
            "success" | "error" | "cancelled" | "interrupted"
        ) {
            return emit(&value, cli.format);
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "export.wait: timed out after {timeout_ms}ms waiting for '{id}' (status={status})"
            ));
        }
        std::thread::sleep(Duration::from_millis(interval_ms));
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
    use clap::CommandFactory;
    use serde_json::json;

    /// Catches a malformed clap tree (duplicate flags, bad defaults) that would
    /// otherwise only panic the first time a user reached that subcommand.
    #[test]
    fn the_argument_tree_is_well_formed() {
        Cli::command().debug_assert();
    }

    mod branch_ops_json {
        use super::*;

        #[test]
        fn accepts_an_array() {
            assert!(read_ops_json(Some(r#"[{"op":"cutAdd","start":1,"end":2}]"#), false).is_ok());
        }

        #[test]
        fn rejects_a_bare_object() {
            assert!(read_ops_json(Some(r#"{"op":"cutAdd"}"#), false).is_err());
        }

        #[test]
        fn rejects_unparseable_json() {
            assert!(read_ops_json(Some("[not json"), false).is_err());
        }

        #[test]
        fn rejects_both_sources_at_once() {
            assert!(read_ops_json(Some("[]"), true).is_err());
        }

        #[test]
        fn rejects_neither_source() {
            assert!(read_ops_json(None, false).is_err());
        }
    }

    mod branch_cli {
        use super::*;

        fn parse(args: &[&str]) -> Cli {
            Cli::try_parse_from(args).expect("parse")
        }

        #[test]
        fn append_reads_the_expected_seq_guard() {
            let cli = parse(&[
                "recast",
                "branch",
                "append",
                "p.recast",
                "--branch",
                "a1",
                "--idem-key",
                "k1",
                "--ops",
                "[]",
                "--expect-seq",
                "3",
            ]);

            let Command::Branch {
                action: BranchAction::Append { expect_seq, .. },
            } = cli.command
            else {
                panic!("expected branch append");
            };
            assert_eq!(expect_seq, Some(3));
        }

        #[test]
        fn apply_requires_a_writer_id() {
            let result =
                Cli::try_parse_from(["recast", "branch", "apply", "p.recast", "--branch", "a1"]);

            assert!(result.is_err());
        }
    }

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
