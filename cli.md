# Recast CLI — Reference

> The `recast` CLI ships in two forms from one binary:
>
> - **Headless** — invoked as `recast <verb>` in a terminal; reads args
>   from `argv`, runs one verb, prints JSON/YAML, exits.
> - **GUI** — invoked with no args; runs the Tauri app (or auto-focuses the
>   already-running instance via `tauri-plugin-single-instance`).
>
> Most verbs route through the running GUI app over a local socket
> (`control.rs`). The CLI is the surface an AI agent or shell script
> uses to drive Recast; every GUI button has an equivalent verb.
>
> Source: `apps/desktop/src-tauri/src/cli.rs`, `control.rs`,
> `commands/editor.rs`. Architecture: `apps/desktop/docs/architecture.md`.

---

## Getting started

### Install the CLI

By default the GUI app installs the CLI on first launch. To install
manually:

```bash
recast install              # adds to PATH (Windows: HKCU\Environment\Path;
                            # macOS / Linux: symlinks to ~/.local/bin/recast
                            # and idempotently appends a guarded PATH line)
recast status               # confirm resolution works in a new terminal
recast uninstall            # remove from PATH (also reverts any rc-file edits)
```

Per-platform mechanics live in `commands/path_install.rs`:

- **Windows** — appends the binary's folder to `HKCU\Environment\Path`
  (`REG_EXPAND_SZ`) and broadcasts `WM_SETTINGCHANGE` so live shells
  pick it up without a re-login.
- **macOS / Linux** — symlinks to `~/.local/bin/recast` AND
  idempotently injects a guarded `PATH` line into whichever rc files
  exist (`~/.zprofile`/`~/.zshrc`/`~/.bash_profile`/`~/.bashrc`/
  `~/.profile`). The block is bounded by
  `# >>> recast cli >>>` / `# <<< recast cli <<<` markers so
  `uninstall` reverts byte-for-byte.

### Global flags

Every verb accepts these before the subcommand:

| Flag | Effect |
|------|--------|
| `-f`, `--format yaml\|json` | Override the default renderer (YAML at a terminal, JSON when piped). |
| `--thumbnails` | Include base64 thumbnails in `displays list` / `windows list` (off by default — they're large). |
| `--no-launch` | Don't auto-start Recast for control commands; fail with `Recast is not running (...)` if the GUI isn't up. |
| `--timeout-ms <N>` | Milliseconds to wait for the app + control server on auto-launch. Default 8000. |

### Output format

Two renderers, same in-memory JSON:

- **YAML** — human-readable, the default when stdout is a terminal.
- **JSON** — single line, the default when piped/captured or `-f json` is passed.

Errors go to stderr and exit non-zero. A trailing verbose new-line is
included for human readers; pipe-to-file consumers strip it.

---

## Verb tree (at a glance)

```
recast devices list                # one payload: displays + windows + mics + cameras
recast displays list               # displays (--thumbnails for previews)
recast windows list                # capturable windows (--pid / --app / --title filters)
recast mics list                   # microphone input devices
recast cameras list [--validate]    # cameras (probe on demand with --validate)
recast capabilities                # capability matrix
recast doctor                      # capabilities + one-line readiness summary
recast status                      # is the GUI app running? + recording state

recast rec { start | stop | pause | resume | status } [--timeout DURATION]
recast select { screen N | window N | region X,Y,W,H | mic <id|default|none>
              | camera <id|none> }     # stage the next recording
recast set { system-audio on|off | fps N | quality auto|balanced|high|pristine
          | countdown N|off }          # tweak the staged intent
recast selection { show | reset }    # inspect / reset the staged CaptureIntent
recast profile { list | show <id> | use <id> }
recast screenshot { display N | window N | app } [--out PATH] [--max PX] [--full] [--base64]
recast screen-read <PATH>          # on-device OCR of a video → structured timeline
recast transcribe --input X --model Y [--language Z] [--out PATH]
recast watch [--events rec,selection,profiles,editor,export]
recast install / recast uninstall    # install the CLI to PATH

recast project { open | show | timeline | zoom-regions | annotations
               | lock | unlock | patch } <PATH> ...
recast editor ...                    # see "Editor control" below
recast branch { create | list | append | diff | show
              | truncate | discard | apply } <PATH> ...   # propose edits for review
recast export { list | show | start | cancel | wait } ...
recast --help                       # all of the above + global flags
```

---

## Recording lifecycle

The recording verbs don't talk to the editor — they only drive
`RecordingManager`. Auto-launches the GUI by default.

```bash
recast status
recast rec start --screen 2 --mic default --system-audio off --timeout 30s
recast rec pause
recast rec resume
recast rec stop                                    # {"projectPath":"<output>/exports/foo.recast"}
recast rec status                                  # {"recording":bool,"paused":bool}
```

### `recast select` — stage the next recording source

```bash
recast select screen 2                       # display id 2 (from `recast displays list`)
recast select window 9                       # window id 9
recast select region 100,200,1280,720        # X,Y,W,H in physical pixels
recast select mic default                    # system default mic
recast select mic usb-1                      # specific mic id
recast select mic none                       # disable mic
recast select camera Webcam                   # enable webcam
recast select camera none                     # disable webcam
```

### `recast set` — tweak options on the staged intent

```bash
recast set system-audio off    # or `on`
recast set fps 60
recast set quality balanced    # auto | balanced | high | pristine
recast set countdown 3         # pre-roll seconds, or `off`
```

### `recast selection` — inspect/reset

```bash
recast selection show        # print the staged CaptureIntent JSON
recast selection reset       # back to defaults (no source, system-audio on)
```

### `recast profile` — saved recording presets

Persisted to `recast_profiles.json`; the panel reads the same store.

```bash
recast profile list          # every saved profile
recast profile show <id|name>
recast profile use <id|name> # apply to the staged intent
```

---

## Editor control — full timeline + everything else

Read-only verbs (`project {open,show,timeline,zoom-regions,annotations}`)
were the first surface. Mutate-side landed after — every mutating verb
acquires the project's per-instance **write-lock**, runs
`validate_render_state` against `VideoMetadata.duration`, persists
via `save_project_edits`, and broadcasts `editor-state:changed` so the
GUI's editor store rehydrates on the next focus.

### Acquisition / release

```bash
# Acquire (the GUI auto-acquires when it opens the project).
recast project lock <PATH> --as agent --writer-id agent:claude-123

# Release (the GUI releases on project close; the CLI on verb return).
recast project unlock --writer-id agent:claude-123

# Force-evict (use sparingly — wipes any in-flight GUI session).
recast project unlock --force --writer-id agent:claude-123
```

Locked, the next mutate verb returns:

```
editor_locked: project '/path/foo.recast' held by 'ui:user' (acquired 12345 ms ago);
use `recast project unlock --force` to reclaim or wait 60s for TTL.
```

### Read-side (`recast project …`)

```bash
recast project open <PATH>           # EditorDocument — every field the GUI uses
recast project show <PATH>           # RenderState (the edits.json) verbatim
recast project timeline <PATH>      # derived trim/cuts/segments/output duration
recast project zoom-regions <PATH>   # list every zoom region
recast project annotations <PATH>    # list every annotation (id, kind, start, end)
```

`project timeline` payload:

```yaml
sourceDuration: 60.0
trimStart: 0.0
trimEnd: 60.0
trimmedDuration: 60.0
outputDuration: 45.2
cuts: [{ start: 12.5, end: 14.0 }]
keptSegments:
  - { start: 0.0, end: 12.5, speed: 1.0 }
  - { start: 14.0, end: 60.0, speed: 1.0 }
splitPoints: []
```

### Targeted mutators (`recast editor …`)

Each verb is shaped identically (set / add / remove / list
subcommands). The lock + validator + event emission are wired through
a single helper so all verbs look and feel the same.

#### Universal mutator — `recast editor set`

Use the universal mutator for **any scalar/struct field** in
`RenderState`. Dotted paths (`borderRadius`, `cursorSize`,
`audioSettings.volume`, `watermarkSettings.opacity`) walk into nested
groups. For **array fields where you want to add or remove entries**,
use the targeted verbs (cut / zoom / split-point / speed / animations
/ annotations) instead.

```bash
recast editor set <PATH> --field borderRadius --value 8 \
    --writer-id agent:claude-123

recast editor set <PATH> --field outputAspect --value '"16:9"' \
    --writer-id agent:claude-123       # string values are JSON-quoted

recast editor set <PATH> --field audioSettings.volume \
    --value 60 --writer-id agent:claude-123

recast editor set <PATH> --field watermarkSettings \
    --value '{"enabled":true,"opacity":80,"position":"bottom-right"}' \
    --writer-id agent:claude-123

recast editor set <PATH> --field cursorSmoothing --value 25 \
    --writer-id agent:claude-123
```

`--value` accepts a JSON value: number, true/false, string (quote inside
the flag), array, or object.

#### Whole-state patch — `recast project patch`

When the targeted verbs don't cover what you want in one shot — or
when an LLM generates a full `RenderState` — round-trip the whole
thing through `project patch`:

```bash
recast project show /path/foo.recast > edits.json
$EDITOR edits.json            # or have an LLM generate one
recast project patch /path/foo.recast --from-file edits.json \
    --writer-id agent:claude-123
```

`--from-stdin` reads JSON from stdin. The payload must be a full
`RenderState` (the same camelCase shape `project show` prints).
The `#[serde(flatten)]` field round-trips JS-only keys, so a JSON
copied straight out of `project show` and edited is a safe starting
point.

#### Trim

```bash
recast editor trim <PATH> --start 0 --end 60 \
    --writer-id agent:claude-123
# { "trimStart": 0.0, "trimEnd": 60.0 }
```

#### Cuts on the timeline

```bash
recast editor cut add <PATH> --start 12.5 --end 14.0 \
    --writer-id agent:claude-123
# { "added": { "start": 12.5, "end": 14.0 } }

recast editor cut list <PATH>
# [{ "index": 0, "start": 12.5, "end": 14.0 }]

recast editor cut remove <PATH> --index 0 --writer-id agent:claude-123
# { "removed": { "start": 12.5, "end": 14.0 } }

# Fallback: address by (start, end) instead of index.
recast editor cut remove <PATH> --start 12.5 --end 14.0 \
    --writer-id agent:claude-123
```

#### Zoom regions

```bash
recast editor zoom add <PATH> \
    --start 30 --end 45 \
    --scale 1.75 \
    --center-x 0.5 --center-y 0.5 \
    --ramp-in 0.35 --ramp-out 0.35 \
    --hidden false \
    --writer-id agent:claude-123
# { "index": 0, "start": 30.0, "end": 45.0 }

recast editor zoom list <PATH>
# [{ "index": 0, "start": 30.0, "end": 45.0, "scale": 1.75,
#    "centerX": 0.5, "centerY": 0.5, "rampIn": 0.35, "rampOut": 0.35, "hidden": false }]

recast editor zoom remove <PATH> --index 0 --writer-id agent:claude-123
```

#### Split markers

```bash
recast editor split-point add <PATH> --at 12.0 \
    --writer-id agent:claude-123
# { "added": 12.0 }

recast editor split-point list <PATH>
# [12.0]

recast editor split-point remove <PATH> --at 12.0 \
    --writer-id agent:claude-123
# { "removed": 12.0 }
```

#### Per-segment speed

```bash
recast editor speed set <PATH> \
    --segment-start 12.0 --rate 2.0 \
    --writer-id agent:claude-123
# { "segmentStart": 12.0, "rate": 2.0 }

recast editor speed list <PATH>
# [{ "start": 12.0, "speed": 2.0 }]

recast editor speed remove <PATH> \
    --segment-start 12.0 --writer-id agent:claude-123
# { "removed": 12.0 }
```

#### Scene animations (per-segment entrance/exit)

```bash
# Animate the start of segment at t=12 — fade-in over 600ms, no outro.
recast editor animations add <PATH> --start 12.0 \
    --in '{"kind":"fade","durationMs":600,"easing":{"x1":0.25,"y1":0.1,"x2":0.25,"y2":1.0}}' \
    --writer-id agent:claude-123
# { "start": 12.0 }

# A slide-in from the left paired with a fade-out.
recast editor animations add <PATH> --start 12.0 \
    --in  '{"kind":"slide","durationMs":500,"dir":"left","intensity":0.4}' \
    --out '{"kind":"fade","durationMs":400}' \
    --writer-id agent:claude-123

recast editor animations list <PATH>
# [{ "start": 12.0, "in": {...}, "out": {...} }]

recast editor animations remove <PATH> --start 12.0 \
    --writer-id agent:claude-123
```

`kind` ∈ `fade` | `slide` | `scale` | `pop`. `dir` ∈
`left` | `right` | `up` | `down`. Intensity is unit-fraction (0..1).

#### Annotations on the timeline

Annotations carry stable `id`s; create / update / remove via `--id`.

```bash
# Add a rectangle highlighting a clip region.
recast editor annotations add <PATH> \
    --kind rect \
    --geometry '{"x":0.1,"y":0.1,"w":0.3,"h":0.2,"radius":0.02}' \
    --start 0 --end 10 --opacity 1.0 \
    --name "intro-pulse" --id "rect-1" \
    --writer-id agent:claude-123
# { "id": "rect-1" }

# Add an arrow. arrow takes (x1,y1) → (x2,y2) in UV.
recast editor annotations add <PATH> \
    --kind arrow \
    --geometry '{"x1":0.2,"y1":0.8,"x2":0.6,"y2":0.2,"headSize":0.2}' \
    --start 2.5 --end 4.0 \
    --writer-id agent:claude-123

# Add a privacy blur over a region.
recast editor annotations add <PATH> \
    --kind blur \
    --geometry '{"x":0.4,"y":0.4,"w":0.2,"h":0.1,"strength":0.7,"variant":"solid","tintColor":"#000000"}' \
    --start 12 --end 18 \
    --writer-id agent:claude-123

# Update an existing annotation by id (any subset of top-level fields).
recast editor annotations update <PATH> --id "rect-1" \
    --patch '{"end":14.5,"opacity":0.6}' \
    --writer-id agent:claude-123

recast editor annotations list <PATH>
# [{ "id": "rect-1", "kind": "rect" }, { "id": "arrow-...", "kind": "arrow" }, ...]

recast editor annotations remove <PATH> --id "rect-1" \
    --writer-id agent:claude-123
# { "removed": "rect-1" }
```

Per-kind `--geometry` schema (UV space 0..1, all numeric fields
required unless noted):

| Kind | Fields |
|------|--------|
| `rect` | `x`, `y`, `w`, `h`, `radius` (default 0) |
| `ellipse` | `x`, `y`, `w`, `h` |
| `arrow` | `x1`, `y1`, `x2`, `y2`, `headSize` (default 0.2) |
| `blur` | `x`, `y`, `w`, `h`, `strength` (default 0.5), `variant` (`solid`\|`tint`, default `solid`), `tintColor` (default `#000000`), `radius` (default 0) |
| `image` | `x`, `y`, `w`, `h`, `path`, `opacity` (default 1), `radius` (default 0) |
| `text` | `x`, `y`, `w`, `h`, `content`, `fontFamily`, `fontSize` (default 16), `fontWeight` (default 400), `color`, `align`, `lineHeight` (default 1.2) |

### Validation gate

Every mutate verb runs through
`validate_render_state(&RenderState, source_duration)`. Errors return
`Vec<ValidationIssue>` where each item is `{ field, reason }` so an
agent can fix a JSON write in one pass:

```json
[
  {"field":"zoomRegions/0/scale","reason":"scale_out_of_range"},
  {"field":"cuts/1","reason":"cut_overlap"},
  {"field":"borderRadius","reason":"border_radius_out_of_range"}
]
```

Reasons currently in scope:

| Reason | Field example | Trigger |
|--------|---------------|---------|
| `non_negative` | `trimStart` | negative seconds |
| `finite` | `trimEnd` | NaN / ±∞ |
| `trim_end_before_start` | `trimEnd` | `trim_end < trim_start` |
| `trim_end_exceeds_source` | `trimEnd` | beyond the source's duration |
| `cut_end_before_start` | `cuts/N/end` | zero- or negative-length cut |
| `cut_out_of_trim` | `cuts/N` | outside `(trim_start, trim_end)` |
| `cut_overlap` | `cuts/N` | two cuts intersect |
| `zoom_out_of_trim` | `zoomRegions/N` | zoom region outside trim |
| `zoom_end_before_start` | `zoomRegions/N` | zero-length zoom |
| `scale_out_of_range` | `zoomRegions/N/scale` | not in `[1.0, 3.0]` |
| `center_out_of_range` | `zoomRegions/N/center` | UV center outside `[0..1]` |
| `ramp_negative` | `zoomRegions/N/ramp` | `ramp_in < 0` or `ramp_out < 0` |
| `annotation_out_of_trim` | `annotations/N` | outside trim |
| `annotation_end_before_start` | `annotations/N` | zero-length annotation |
| `opacity_out_of_range` | `annotations/N/opacity` | not in `[0..1]` (or `[0..100]` for cursor highlight) |
| `position_out_of_range` | `annotations/N/position` | UV outside `[0..1]` |
| `radius_out_of_range` | `annotations/N/radius` | not in `[0..0.5]` (where defined) |
| `points_out_of_range` | `annotations/N/points` | arrow endpoints UV outside `[0..1]` |
| `strength_out_of_range` | `annotations/N/strength` | blur strength outside `[0..1]` |
| `border_radius_out_of_range` | `borderRadius` | not in `[0..50]` |
| `padding_out_of_range` | `padding` | not in `[0..20]` (percent) |
| `cursor_size_negative` | `cursorSize` | `< 0` |
| `cursor_smoothing_out_of_range` | `cursorSmoothing` | not in `[0..100]` (percent) |
| `cursor_highlight_opacity_out_of_range` | `cursorHighlightOpacity` | not in `[0..100]` |
| `speed_non_positive` | `segmentSpeeds/N/speed` | `<= 0` or non-finite |

Rename any reason and update the agent that branches on it in the same
commit.

---

## Branches — propose edits without touching the project

`recast editor …` writes straight into the `.recast`. `recast branch …`
instead journals typed ops against the render state they forked from,
so an agent can propose a whole edit without holding the lock, without
rewriting the bundle per op, and without a human losing their work.

```
recast branch create  <PATH> --branch a1 --author agent:claude [--label "tighten intro"]
recast branch list    <PATH>
recast branch append  <PATH> --branch a1 --idem-key k1 --ops '<JSON array>' [--expect-seq N]
recast branch append  <PATH> --branch a1 --idem-key k1 --from-stdin
recast branch diff    <PATH> --branch a1        # field-level changes
recast branch show    <PATH> --branch a1        # the full render state it would produce
recast branch truncate <PATH> --branch a1 --seq 3   # drop everything after seq 3
recast branch discard <PATH> --branch a1
recast branch apply   <PATH> --branch a1 --writer-id ui:me
```

### Ops

`--ops` takes a JSON array of the same operations the targeted verbs
perform. The `op` tag is the camelCase verb name:

```json
[
  { "op": "trim", "start": 1.0, "end": 30.0 },
  { "op": "cutAdd", "start": 4.0, "end": 5.0 },
  { "op": "zoomRemove", "index": 0 },
  { "op": "speedSet", "segmentStart": 12.0, "rate": 1.5 },
  { "op": "annotationRemove", "id": "rect-1" },
  { "op": "set", "field": "borderRadius", "value": 12 }
]
```

Every op in one `append` lands together or not at all.

### Guarantees

| Concern | Behaviour |
|---------|-----------|
| Retry after a dropped socket | Re-sending the same `--idem-key` is a no-op; the response has `"recorded": false` and the original `seq` |
| Another writer got in first | `--expect-seq` mismatch is rejected before anything is recorded |
| An op that cannot apply | Rejected at `append` time (the branch is replayed first), not at review time |
| Project edited since the fork | `branch apply` fails with `branch forked from <hash> but the project is now at <hash>` |
| A branch that grows unbounded | Auto-compacts to a single `replace` op past 512 entries; the fork point is preserved |
| Bundle rewrites | One, on `apply`. Never on `append` |

Journals live under `<app_data_dir>/branches/<project-key>/<branch>.json`,
not beside the `.recast`, so the temp-dir sweeper cannot reclaim pending
work. Listen for changes with `recast watch --events editor`.

---

## Export

The export queue lives in SQLite (`export_jobs`) and feeds a single
serial worker. Each verb maps onto a row in `ExportJobDto`.

### `recast export start` — every flag

```bash
recast export start <PATH> \
    --format mp4|webm|gif                            # default: mp4
    --quality auto|balanced|high|pristine            # default: balanced
    --speed fast|balanced|quality                   # encoder effort axis (orthogonal to quality)
    --fps <N>                                        # output frame rate (≤ source fps); ignored for GIF
    --burn-captions                                  # bake transcript into the video
    --caption-sidecar vtt|srt                       # write a sidecar subtitles file
    --gif-fps <N>                                    # GIF frame rate override (format=gif only)
    --gif-quality low|medium|high                   # GIF palette: 64 / 128 / 256 colors
    --gif-loop infinite|once|<n>                     # GIF loop count
    --gif-dither bayer|sierra2|none                 # GIF dither algorithm
    --writer-id agent:claude-123
# {"exportId":"cli-...","format":"mp4","quality":"balanced",
#  "speed":"fast","fps":30.0,"burnCaptions":true,
#  "gifSettings":{"fps":12,"quality":"medium","loop":"infinite","dither":"bayer"}}
```

The render-state patch is picked up via `--renderState <JSON>`
(advanced; most callers want `recast editor patch` instead).

Validation: `validate_render_state` runs at the IPC door (in
`commands/export_queue.rs` — independent of this CLI verb) so a bad
state fails immediately rather than mid-encode.

### `recast export list` / `show` / `cancel` / `wait`

```bash
recast export list                          # every job (queued/running/terminal)
recast export show <JOB_ID>
recast export cancel <JOB_ID>               # queued → removed; running → signalled
recast export wait <JOB_ID>                 # polls every --interval until terminal
    --timeout 10m
    --interval 1s
```

`export list` row shape:

```yaml
id: cli-1234-1700000000000
filename: foo.recast
filePath: /path/foo.recast
status: queued | running | success | error | cancelled | interrupted
phase: preparing | encoding | finalizing | cancelling
progress: 0..100
path: /output/exports/foo.mp4        # present iff success
error: <message>                      # present iff error
createdAt: 1700000000000
startedAt: 1700000000123              # present iff started
finishedAt: 1700000001234             # present iff terminal
```

---

## Capture / OCR / transcription

### `recast screenshot`

Lets an agent see on-screen state and decide when a step is done or
what to do next.

```bash
recast screenshot display 2 --out /tmp/agent-display-2.png --max 1280
recast screenshot window 5 --full                    # native resolution, no cap
recast screenshot app --window main --base64 > /tmp/agent-app.png
```

`--base64` includes the data URI in the response payload alongside
the file path, so a tool that captures the stdout can embed the
image inline without round-tripping a file.

### `recast screen-read <PATH>`

On-device OCR of a video into a structured timestamped text timeline
(`Vec<ScreenTextSegment>`). Useful for an agent that needs to know
"what happened" in a recording without listening to narration.
Requires the OCR feature in the build.

```bash
recast screen-read /path/foo.mp4
```

### `recast transcribe`

Offline transcription against a downloaded `.gguf` model. Doesn't
need the GUI to be running — works in CI / release smoke test.

```bash
recast transcribe --input audio.wav --model whisper-base-Q5_K_M.gguf \
    --language en --out transcript.json   # omit --out for stdout
```

The CLI flags are part of the CI release smoke test contract; rename
the verb or any flag and update
`scripts/release/smoke-test-transcription.ps1` in the same commit.

---

## Streaming (`recast watch`)

Opens a long-lived JSONL stream of backend events on stdout until
interrupted (Ctrl-C).

```bash
recast watch --events rec,selection,profiles,editor,export
```

| Group | Events |
|-------|--------|
| `rec` | `recording:started`, `recording:stopped` |
| `selection` | `capture-intent:changed` |
| `profiles` | `recording-profiles:changed` |
| `editor` | `editor-session:changed`, `editor-state:changed` |
| `export` | `export-state`, `export-jobs-changed` |

Each frame is one JSON object per line:

```json
{"event":"recording:started","data":{}}
{"event":"editor-state:changed","data":{"path":"/path/foo.recast"}}
```

A heartbeat `{"event":"ping"}` fires every 15s; ignore it (or use as
a liveness probe).

---

## CLI installer — `recast install` / `uninstall`

`recast install` is idempotent. After running, open a new terminal so
the shell re-reads `PATH`.

Setting toggles in the GUI: **Settings → Command line tool**.

- `Install the recast command` — calls `install_cli`.
- `Auto-install on first launch` — controlled by `AppConfig.cli_auto_install`. Defaults to on. Disable to stop future auto-attempts; the install verb is still callable.
- `Modified shell config: ~/.zshrc ~/.profile` — pill chips showing which rc files carry the `recast` block (macOS / Linux only).

`recast install --no-launch` writes to PATH without touching the
running app. `recast uninstall --no-launch` removes it. `--no-launch`
matters when scripting in a CI runner that has the binary on disk but
no GUI to talk to.

---

## Locking semantics

`EditorSession` is a `parking_lot::RwLock<EditorSession>` in
`AppState`. Acquire calls go through `try_acquire_write`; only the
current holder (matching `writer_id`) can release. A stale lock (no
activity for `EditorSession::TTL_MS` = 60s) auto-reclaims.

| Holder | Other side experience |
|--------|------------------------|
| GUI user holds write | CLI mutate verbs return `editor_locked: … held by 'ui:<user>' (acquired Nms ago)` |
| Agent holds write | GUI shows banner *"Agent `<writer_id>` is editing this project — your edits are paused"* + disables mutating inputs (preview scrubbing + watch still work) |

Re-acquiring is free for the writer that already holds the lock: the
same `writer_id` refreshes the activity stamp and keeps the original
`acquired_at_ms`, so a multi-step agent edit never blocks on itself.
Only a *different* `writer_id` sees `editor_locked`.

`recast branch …` never takes the lock. Only `branch apply` does.

Crash safety:
- Every successful acquire / release / mutation persists the snapshot
  to `<app_data_dir>/recast_session.json` (atomic write).
- On next boot, the snapshot is loaded only if the holder's PID is
  still alive (`kill(pid, 0)` on Unix, `OpenProcess` on Windows).
- A stale snapshot is cleared without prompting.

Listen for transitions via `recast watch --events editor`.

---

## Recipes

### "Record a 15-second region capture, trim it, export as MP4"

```bash
# 1. Stage
recast select region 100,200,1280,720
recast set fps 60
recast set system-audio off

# 2. Record with a 15s auto-stop
recast rec start --region 100,200,1280,720 --timeout 15s
# ... 15s of recording ...

# 3. Open the resulting project, trim a couple of seconds off the tail
PROJECT=$(recast selection show | jq -r .lastFilePath)   # or your own
recast editor trim "$PROJECT" --start 0 --end 13 \
    --writer-id agent:claude-123

# 4. Export and wait for the file
JOB=$(recast export start "$PROJECT" --format mp4 --quality high \
        --writer-id agent:claude-123 | jq -r .exportId)
recast export wait "$JOB" --timeout 5m
# {"path":"/output/exports/foo.mp4", ...}
```

### "An AI agent introspects a recording and applies edits"

```bash
PROJECT=/output/exports/foo.recast

# Snapshot the current state.
recast project show "$PROJECT" > edits.json

# (LLM reads edits.json + the plan, writes edits.new.json.)

# Apply, with a writer-id that identifies the agent.
recast project patch "$PROJECT" \
    --from-file edits.new.json \
    --writer-id agent:claude-123
# `applied: true` on success; `validation failed: [...]` otherwise.
```

### "Add a zoom region at click-feedback time"

```bash
# User just dragged a zoom into the editor; the agent wants to keep
# iterating without going through the GUI. The RenderState is shared
# (#[serde(flatten)] passthrough) so a small targeted change sticks.

recast editor zoom add "/path/foo.recast" \
    --start 30 --end 45 \
    --scale 1.75 \
    --center-x 0.5 --center-y 0.5 \
    --writer-id agent:claude-123
```

### "Build a callout blur over an SSN region"

```bash
recast editor annotations add "/path/foo.recast" \
    --kind blur \
    --geometry '{"x":0.45,"y":0.42,"w":0.18,"h":0.05,"strength":0.85,"variant":"solid","tintColor":"#1f2937"}' \
    --start 12 --end 18 \
    --id ssn-redact \
    --writer-id agent:claude-123
```

### "Drive the editor from a script that just lost the lock"

```bash
recast project lock "$PROJECT" --as agent --writer-id agent:builder

# Acquired. Do work; the GUI banner shows your writer-id mid-flight.

recast editor cut add "$PROJECT" --start 4.0 --end 4.5 \
    --writer-id agent:builder
recast editor cut add "$PROJECT" --start 10.0 --end 10.5 \
    --writer-id agent:builder
recast editor zoom add "$PROJECT" --start 4.0 --end 10.5 --scale 2.0 \
    --writer-id agent:builder

recast project unlock --writer-id agent:builder
```

If the GUI user grabbed the lock first:

```bash
# `editor_locked: … held by 'ui:bob'` — don't fight, surface to the caller.
# The verb returns immediately; no implicit retry/wait.
```

### "Tune export quality without re-recording"

```bash
recast export start /path/foo.recast \
    --quality pristine \
    --speed quality \
    --burn-captions \
    --gif-fps 12 --gif-quality high --gif-loop once \
    --writer-id agent:claude-123   # format=gif defaults implicit
```

---

## Backwards-compat notes

- `recast watch --events` keeps the comma-separated flag for the
  pre-existing groups (`rec`, `selection`, `profiles`); the new
  `editor` / `export` groups plug into the same flag.
- `recast install` / `recast uninstall` no longer just symlink — on
  Unix they also edit rc files. The `message` field still confirms
  what changed.
- The validation gate returns `Vec<ValidationIssue>` as JSON;
  older scripts that grep for `validation_failed` still work via the
  wrapper message text.

---

## See also

- `apps/desktop/docs/architecture.md` — recording + editor + render
  pipeline architecture (the model this CLI drives).
- `apps/desktop/src-tauri/src/cli.rs` — single source of truth for verb
  shape and parsing.
- `apps/desktop/src-tauri/src/control.rs` — the dispatch table the
  CLI channels its live verbs through.
- `apps/desktop/src-tauri/src/commands/editor.rs` —
  `validate_render_state` + `derive_project_timeline` +
  `patch_render_state` helpers.
- `apps/desktop/src-tauri/src/commands/editor_session.rs` — the
  `EditorSession` lock + persistence helpers.
