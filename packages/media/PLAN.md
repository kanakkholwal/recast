# Migration Plan — Editor preview on MediaBunny

**Status:** Pre-PR-A. Scaffold + requirements doc land together.
**Scope:** desktop video editor (preview pipeline) + shared MediaBunny
helpers in `@recast/media`. Screenshot editor removed from desktop; web's
`mp4-muxer` legacy path untouched. Rust export pipeline untouched;
`EnqueueExportRequest` IPC payload stays byte-stable.

---

## Goals

- Replace `apps/desktop/src/lib/playback/{webcodecs-source,webcodecs-worker,mp4-demux}.ts`
  + the `mp4box` dependency with a MediaBunny-backed pipeline living in
  `@recast/media`.
- Reuse `apps/web/src/lib/tools/mb.ts` helpers (no duplication) by
  relocating them into `packages/media` behind a re-export shim.
- Worker-from-day-one: decode + demux off the main thread, AudioWorklet
  for sample-accurate audio.
- Hit every budget in `REQUIREMENTS.md §3`, with regressions caught by
  `packages/media/test/perf/budgets.test.ts`.

## Non-goals

- Rust export pipeline (`apps/desktop/src-tauri/src/commands/export/*`).
- `packages/application/screenshot-editor/*` (web keeps its `mp4-muxer` path).
- `apps/desktop/src/lib/timeline/filmstrip-*` (separate hot path; revisit later).
- Desktop `/tools` route mirroring apps/web's `/tools`.

---

## PR sequence

Six PRs. Each on a branch. Conventional commits. Single concern.
Maintainer owns merges per AGENTS.md §2.

| # | Branch | Goal | Behavior change |
|---|---|---|---|
| **PR-A** | `feat/media-scaffold` | Scaffold `packages/media` + REQUIREMENTS.md + perf-budget tests skeleton | none |
| **PR-B** | `refactor/media-relocate` | Relocate apps/web's `mb.ts` / `encoders.ts` / `worker-protocol.ts` into `@recast/media` behind a re-export shim | none |
| **PR-C** | `chore/screenshot-editor-remove` | Remove screenshot editor from desktop (route, IPC, Rust command) | removes dev-only nav link + unused-by-anything-else route + IPC + Rust command |
| **PR-D** | `feat/media-playback-source` | `PlaybackSource` worker + adapter (feature-flagged; no behavior change) | none |
| **PR-E** | `feat/media-cache-audio` | IndexedDB cache + AudioWorklet scheduler (legacy WebCodecs path still togglable) | adds cache + audio scheduler |
| **PR-F** | `feat/media-editor-migration` | Flip `VideoPreview.svelte` + delete legacy WebCodecs files | swaps preview pipeline |

PR-F is **merge-blocked on the cut-jump parity fixture being green.**

---

## Architecture (target state)

```
Main thread                          Worker: media-worker.ts           Worker: audio-worklet.ts
─────────────                        ──────────────────────────        ──────────────────────
UI / state / rAF                     MediaBunny Input                  Scheduled AudioBufferSourceNodes
VideoPreview.svelte ──seek──►        • demux (mp4/mov/webm)            aligned to scrub events from main
       │                             • WebCodecs VideoDecoder          backpressure-aware via messages
       │                             • GOP cache (LRU by bytes)
       │                             • IndexedDB-backed decoded-frame cache
       │                                   │
       ▼                                   ▼
CanvasSource / CanvasSink  ◄── transferable VideoFrame
       │
       ▼
Compositor (WebGL2) → texture upload → rAF → screen
       │
       ▼
Overlay lanes (cursor sprite, zoom, annotations, captions)
via @recast/ui surface — same shader/uniform pipeline as today
```

Hard rules:

- No transferable `VideoFrame` ever closes on the producer side before the
  consumer paints it (refcounting rule from
  `apps/desktop/src/lib/playback/webcodecs-source.ts:22`).
- Demux + decode in worker; main thread only consumes finished frames.
- Compositing stays on the main thread (WebGL2 needs the DOM-bound canvas).
  If profiling shows main-thread pressure, migrate to OffscreenCanvas via
  `transferControlToOffscreen` — the seams are already designed for this.
- IndexedDB holds recently-decoded GOPs for re-scrub; LRU-evicted by
  recency × byte budget. Worker reads it; main thread never sees it.
- Range requests: desktop uses Tauri's `asset:` protocol (already used);
  web uses HTTP `Range` against the API. `@recast/media` exposes a
  `StreamSource` adapter for both.

---

## Milestone-by-milestone testing guide

**Run these before opening the next PR.** Every command must be green;
every manual check must pass. Do not skip manual checks — automated tests
do not catch everything (the cut-jump freeze is the canonical example).

### Pre-flight (before any PR)

```bash
pnpm install
pnpm check
pnpm lint
pnpm fmt:check
pnpm test
pnpm build
```

All must be green. If any fails, fix the baseline before starting.

### After PR-A — scaffold

**Commands:**

```bash
pnpm --filter @recast/media build
pnpm --filter @recast/media test         # budgets.test.ts skeleton passes vacuously
pnpm check                                # cross-workspace svelte-check
pnpm test                                 # full vitest suite
pnpm build
```

**Manual:**

- Open `packages/media/package.json`; confirm `mediabunny` is a dep and
  there is no `mp4-muxer` reference anywhere.
- Confirm `packages/media/src/index.ts` re-exports the planned public API
  even if the bodies are stubs.
- Read `packages/media/REQUIREMENTS.md` end-to-end. Flag anything unclear
  before PR-B starts.
- `git grep "mediabunny" apps/` returns zero (consumers do not yet import
  from the new package).

**Gate to PR-B:** all commands green + review of `REQUIREMENTS.md` complete.

### After PR-B — relocate apps/web helpers

**Commands:**

```bash
pnpm check
pnpm --filter recast-web test
pnpm --filter @recast/media test
pnpm --filter recast-web build
pnpm build
```

**Manual:**

- Run `/tools/trim-video` on a sample MP4. Output visually identical (or
  byte-equal) to pre-migration.
- Run `/tools/video-to-gif`, `/tools/audio-to-mp3`, `/tools/compress-video`.
  Output visually unchanged.
- Inspect the bundle: `mediabunny`, `gifenc`, `@breezystack/lamejs`, and
  `fflate` must no longer appear in `apps/web`'s direct deps
  (`apps/web/package.json`).
- `git grep "from 'mediabunny'" apps/` returns zero (all consumers go
  through `@recast/media`).

**Gate to PR-C:** web app conversion tools all produce visually-identical
output to baseline; no `mediabunny` import remains outside `packages/media`.

### After PR-C — remove screenshot editor from desktop

**Commands:**

```bash
pnpm check
pnpm test
cargo fmt --check --manifest-path apps/desktop/src-tauri/Cargo.toml
cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
pnpm tauri build
```

**Manual:**

- `rg "capture_screenshot|screenshot::" apps/desktop/src-tauri/src`
  returns zero matches.
- Sidebar (dev build) no longer shows the "Screenshot" link.
- `pnpm tauri dev` opens; verify the home page renders with no console
  errors.
- `apps/desktop/src-tauri/src/lib.rs` has no `commands::capture_screenshot`
  import.
- `apps/desktop/package.json` no longer lists `@recast/application`.
- `apps/desktop/src/app.css` has no `@source` pointing at
  `@recast/application`.
- `apps/desktop/src/routes/+layout.svelte` does not import
  `@recast/application/styles.css`.
- Run `rg "@recast/application" apps/desktop/src` — should return zero.

**Gate to PR-D:** tauri build green, no screenshot references remain,
manual smoke clean.

### After PR-D — PlaybackSource worker + adapter (feature-flagged)

**Commands:**

```bash
pnpm check
pnpm test
pnpm --filter @recast/media test
cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets -- -D warnings
pnpm tauri build
```

**Manual (feature flag ON):**

- Open the editor on a 1080p recording. Confirm playback starts and looks
  visually identical to the WebCodecs path.
- Open the editor on a 4K recording. Time-to-first-frame ≤ 800 ms.
- DevTools → Performance → record 5 s of playback. Main-thread script
  time p95 ≤ 4 ms (we moved decode to a worker; main thread should be
  near-idle).
- Toggle the feature flag OFF; confirm playback still works (WebCodecs
  path unchanged).
- Toggle ON/OFF repeatedly while playing; no leaks, no stuck frames.

**Manual (feature flag OFF):** identical to current behavior; no
regressions.

**Gate to PR-E:** no measurable regression with flag ON; behavior
identical with flag OFF.

### After PR-E — cache + AudioWorklet

**Commands:**

```bash
pnpm check
pnpm test                                       # includes perf/budgets.test.ts
pnpm --filter @recast/media test
pnpm tauri build
```

**Manual:**

- Open a long (10+ min) recording.
- Scrub to a random point, scrub again, scrub back. Second+ scrubs should
  be cache hits (DevTools → Application → IndexedDB → `@recast/media` →
  `decoded-frames` shows entries).
- Set a cut; cross the cut. Audio stays in sync (no drift over 60 s of
  playback).
- Check memory in DevTools. Decoded-frame buffer stays ≤ 512 MB; IndexedDB
  stays ≤ 2 GB.
- Change Settings → Media → IndexedDB cap to 256 MB. Reload. Cache
  honors the new cap; eviction kicks in on next scrub.
- Cross a cut while paused. Audio does not drift ahead or behind the
  playhead.

**Gate to PR-F:** cache hits observable, audio stays in sync, memory
under cap, cap is user-configurable.

### After PR-F — flip VideoPreview + delete legacy

**This is the merge-blocking milestone.**

**Commands:**

```bash
pnpm check
pnpm test                                              # includes perf/budgets.test.ts AND cut-jump.test.ts
pnpm lint                                              # clippy
pnpm tauri build
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
```

**Cut-jump parity fixture (must be green):**

- 4K @ 60 fps recording, 2 s cut at t = 10 s.
- 50 iterations of `seek(cut.end)` + capture frame.
- p95 latency on the new path must be ≤ baseline (target: ≤ 250 ms).
- Document the new p95 in the PR description.

**Manual smoke (exhaustive):**

- Open a 4K recording from the library. Play, pause, scrub, jump to
  start, jump to end. No jank, no error.
- Open a 1080p recording with 3 cuts. Play across all cuts; verify
  cut-cross latency stays under the budget.
- Open a recording with captions baked in. Verify captions render.
- Open a recording with camera bubble. Verify the bubble tracks
  playback.
- Open a recording with zoom regions. Verify zoom activates correctly.
- Stress: open 5 recordings back-to-back. Memory growth bounded;
  previous recording's resources released.
- Edge: open a corrupt file. Error surfaces cleanly; no white-screen.
- Edge: open a file with no audio. No audio errors.
- Edge: open a recording mid-export (concurrent). No crash.
- Edge: scrub during a decode-ahead preload. No double-decode, no
  duplicated `VideoFrame`s.
- Regression: cut / split / trim operations in the editor (in/out
  handles) still work end-to-end.
- Cross-platform smoke (best effort): Windows, macOS, Linux (the test
  matrix the project already uses).

**Performance fixtures run inside `pnpm test`:**

- `packages/media/test/perf/budgets.test.ts` — frame-to-glass p95,
  seek latencies, bundle size.
- `packages/media/test/perf/cut-jump.test.ts` — cut-cross latency parity.

**Gate to merge:** every command green; every manual check passed; perf
budgets met; cut-jump parity verified; bundle sizes within budget;
no `mediabunny` import outside `packages/media`; no `mp4box` import
remains in `apps/desktop`.

---

## Cross-cutting verification (every PR)

Per AGENTS.md §6 "Definition of done":

- `pnpm check`
- `pnpm lint`
- `pnpm fmt:check`
- `pnpm test`
- `pnpm build`
- `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings` + `cargo test`

Plus the milestone-specific commands listed above.

After **each** PR, run the perf-budget test:

```bash
pnpm --filter @recast/media test -- perf/budgets
```

A regression fails the PR — fix or amend before requesting review.

---

## Risk register

| Risk | Mitigation |
|---|---|
| Phase-3 (PR-F) regresses the cut-freeze fix the original WebCodecs work delivered | Cut-jump parity fixture captured **before** the import flip. Legacy files NOT deleted until baseline met. Run the macOS hang regression (`commands/recording.rs:380`) after every PR. |
| MediaBunny `Input` runs main-thread (no Worker); Tauri WebView2 on Windows may stall on a heavy decode | Worker-from-day-one (PR-D). A/B against today's `webcodecs-worker.ts` on a 4K recording before flipping. |
| Bundle-size growth from MediaBunny vs the tree-shaken mp4box + hand-rolled decoder | Direct imports only. `pnpm build` report before/after each PR. If regression > 50 KB gz, audit `@recast/media` exports — only the slices used by the consumer should be reachable. |
| `CanvasSink.screenshotAtTimestamps` doesn't expose the same "prefetch N frames ahead" hook the scout decoder uses today | Build a `ScoutSource` wrapper in `packages/media` that pre-pulls the next post-cut frame via a second `CanvasSink` instance — same pattern as today, just over MediaBunny. |
| IndexedDB write storms during long scrub sessions cause main-thread jank | Throttle writes; coalesce by GOP; only commit on `requestIdleCallback`. Test on a 10+ min recording. |
| AudioWorklet fallback path (legacy `audio-engine.ts`) drifts from the new path during the fallback window | Each PR that touches audio must run both paths against the same timeline fixture. The parity test lives in `packages/media/test/perf/audio-parity.test.ts`. |
| `apps/desktop/src/lib/timeline/filmstrip-*` not migrated → diverges from the new preview pipeline | Out of scope for this plan. Flagged for follow-up; track as a separate workstream. |

---

## Open questions

None. Confirmed:

- Doc creation in PR-A: **yes** (this file lands with the scaffold).
- AudioWorklet fallback in PR-E: **keep fallback** during the fallback
  window; remove once testing confirms the new path is stable for a
  milestone.
- IndexedDB cap: **2 GB default, user-configurable** in Settings.
- Perf-fixture location: **`packages/media/test/perf/`**.

---

## After each milestone completion — what to test before the next PR

A condensed reminder. Detailed checklists live in each milestone's section
above; this is the quick checklist:

1. **All commands green** (`pnpm check`, `pnpm test`, `pnpm build`,
   `cargo` gates).
2. **Manual smoke** for that milestone passed.
3. **Perf-budget test green** (`pnpm --filter @recast/media test -- perf/budgets`).
4. **No out-of-scope regressions** (editor still works, web tools still
   work, screenshot editor untouched, Rust export untouched).
5. **No `mediabunny` import leaked** outside `packages/media`.
6. **Bundle size within budget** for the target app.
7. **Memory under cap** (DevTools check on a long recording).
8. **No leaks**: open 5 recordings back-to-back; verify resource
   release.

Only when all eight are green do you open the next PR.
