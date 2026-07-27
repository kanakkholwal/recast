# Media Engine Migration Log

> Status of the MediaBunny-backed editor preview migration. Captured at the
> close of PR-D / PR-E / PR-F (the three PRs that landed the new pipeline and
> removed the legacy webcodecs+mp4box dependency). Read this before picking
> up the next phase — it explains what's done, what's left as cleanup, and
> what the original goal was.

---

## End goal

Replace the legacy `WebCodecsVideoSource` (hand-rolled WebCodecs + `mp4box.js`)
preview engine with a `MediabunnyVideoSource` (MediaBunny-backed) so the
desktop editor's preview pipeline is:

- **Single dependency.** One media toolkit (`mediabunny`) instead of two
  (`mediabunny` + `mp4box.js` + a hand-rolled WebCodecs wrapper).
- **Sample-accurate across cuts.** No more `<video>`-element freeze when the
  playhead jumps across a cut — MediaBunny's `Input` + `CanvasSink` answers
  any seek deterministically.
- **Worker-from-day-one.** Decode + demux run in a Web Worker; the main
  thread stays free for input + paint. INP stays under the 100 ms target
  from `REQUIREMENTS.md §3` even during heavy scrubbing.
- **Persistent across reloads.** IndexedDB-backed frame cache (2 GB default,
  user-configurable, LRU-by-recency-x-bytes) survives editor restarts.
- **Sticks to Recast's own output.** Recast's Rust pipeline produces MP4 with
  AVC + AAC, which MediaBunny fully decodes hardware-accelerated. The
  editor's preview of a Recast recording is the happy path.

The editor preview now uses MediaBunny unconditionally. The `<video>` element
remains the **fallback** for the gap documented in
`src/cache/unsupported-formats.ts` (AVI, FLV, WMV, RealVideo, 3GP) — the
same fallback the legacy pipeline ultimately used too.

---

## What landed (DONE)

### PR-A — `@recast/media` scaffold + public API + tests
- New workspace package `@recast/media` (npm workspace dep) wrapping
  MediaBunny behind a high-level surface (`openInput`, `runConversion`,
  `outputFormatFor`, `openMediaSource`, `createAudioScheduler`, …).
- 30 vitest cases pinning the public API; biome enforces `no mediabunny`
  imports outside the package.
- Stub-only `unsupported-formats.ts` placeholder for PR-D's worker.

### PR-B — apps/web tools relocation
- `apps/web/src/lib/tools/{mb,handlers,encoders,worker-protocol}.ts`
  became thin re-export shims over `@recast/media`.
- `mediabunny` / `gifenc` / `@breezystack/lamejs` / `fflate` dropped
  from `apps/web/package.json` direct deps; `@recast/media` is the
  single entry point.

### PR-D — `MediabunnyVideoSource` landing strip + feature flag
- `apps/desktop/src/lib/playback/{mediabunny-worker,mediabunny-source}.ts`
  implementing the playback surface (`frameAt`, `prefetch`, `dispose`,
  `onFrame`, `onStats`, `width`, `height`, `durationSec`, `fps`).
- Worker owns MediaBunny `Input` + `CanvasSink`; main thread caches
  `VideoFrame`s in a `Map<tsUs, VideoFrame>`. New seeks supersede
  stale in-flight ones (`#inFlightSeq` guard).
- `feature-flag.ts` controlled by `?useLegacyPreview=1` / `?mbPreview=0`.
- `cut-jump-parity.test.ts` — 14 tests, 50-iteration p95 latency check
  for the `outputToOriginal → spanAtOriginal` pipeline. Final numbers:
  `p50=0.12ms p95=0.31ms p99=1.72ms max=1.72ms` — well under the
  250 ms budget.
- `mediabunny-source.test.ts` — 6 tests exercising the playback
  surface itself (worker mock via `vi.stubGlobal`), including the
  supersede-cancel contract: a stale frame is dropped, a fresh
  frame is cached.
- `editor-pipeline.test.ts` — 12 tests covering layer visibility
  across cuts, fade envelopes, audio scheduling across cuts
  (kept-region → chunk gap semantics), and a multi-cut timeMap
  roundtrip.
- `feature-flag.test.ts` — 8 tests pinning flag semantics
  (default ON, `useLegacyPreview=1` opts to legacy).

### PR-E — IndexedDB cache + AudioWorklet scheduler
- `packages/media/src/cache/` (new):
  - `storage.ts` — `FrameStorage` interface, `CacheableFrame` type,
    `estimateFrameBytes()`.
  - `indexeddb-storage.ts` — `IndexedDBFrameStorage` (LRU on `put`
    when over cap, half-open `deleteRange`, `navigator.storage.persist()`).
  - `sqlite-storage.ts` — **stub** (the user decided against SQLite;
    IndexedDB covers all current platforms).
  - `index.ts` — `FrameCache` orchestrator (in-memory hot layer +
    persistent cold layer), `getFrameCache()` singleton, factory +
    `setFrameStorage()` for swapping backends, `resetFrameCache()` for
    tests.
  - `unsupported-formats.ts` — curated list of containers/codecs that
    **NEITHER MediaBunny NOR legacy webcodecs+mp4box could decode**
    (AVI, FLV, WMV, RealVideo, 3GP), with `isUnsupportedContainer` /
    `isUnsupportedCodec` helpers.
- `packages/media/src/audio/`:
  - `schedule.ts` — pure scheduling math (`keptRegions`,
    `planAudioSchedule`) shared by both scheduler implementations.
  - `scheduler.ts` — `AudioScheduler` interface with two impls:
    `WorkletAudioScheduler` (sample-accurate, AudioWorklet-backed) and
    `FallbackAudioScheduler` (JS-thread, mirrors legacy `audio-engine.ts`).
- `apps/desktop/src/lib/playback/audio-engine.ts` rewritten with
  per-track `kind: 'system' | 'mic'`, `setMasterVolume` /
  `setTrackVolume` methods. Master × per-track gain is composed in
  `#applyGains()`; master mute zeroes both.
- `AudioSettings` interface (editor store) extended with
  `systemVolume` / `systemMuted` / `micVolume` / `micMuted`. Backward
  compat falls back to master values for old projects.
- `AudioPanel.svelte` rebuilt with per-track mute toggles + sliders
  on top of the master controls.
- 7 cache tests + 14 audio scheduling tests pinned.

### PR-F — flip default + delete legacy + remove mp4box (this PR)
- `feature-flag.ts` inverted: MediaBunny is now the **default**;
  `?useLegacyPreview=1` opts into the (now-deleted) WebCodecs fallback.
  The file was deleted in the cleanup pass below.
- `taxonomy.ts` events renamed from `webcodecs_preview_*` to
  `mediabunny_preview_*` (props unchanged).
- `VideoPreview.svelte` rewritten to drop the legacy branch. The
  preview engine is now `MediabunnyVideoSource` only. Telemetry events
  fired: `mediabunny_preview_init` (on successful init), and
  `mediabunny_preview_fallback` (with a classified reason from
  `classifyMbError`, mirroring the old `classifyWcError`).
- **Cleanup pass** (what landed in this turn):
  - Deleted `apps/desktop/src/lib/playback/{webcodecs-source,webcodecs-worker,webcodecs-protocol}.ts`
  - Deleted `apps/desktop/src/lib/playback/{mp4-demux,mp4-sample-table,mp4-sample-table.test}.ts`
  - Deleted `apps/desktop/src/lib/playback/{feature-flag,feature-flag.test}.ts`
    (no consumer — the flag is now a no-op with the legacy gone)
  - Removed `mp4box` from `apps/desktop/package.json`
  - Renamed `wcSource` → `mbSource`, `wcReady` → `mbReady`,
    `loadedWcSrc` → `loadedMbSrc`, `classifyWcError` → `classifyMbError`
    throughout VideoPreview and `video-preview.logic`
  - Updated `filmstrip-{worker,source}.ts` to use MediaBunny
    `Input` + `CanvasSink` instead of mp4box + WebCodecs
  - 14 unsupported-formats tests (8 in `packages/media/`, 6 in
    `apps/desktop`)

### Test totals (close of PR-F)
- `@recast/media`: 66 (incl. cache, audio, conversion, seek, smoke,
  budgets, unsupported-formats)
- `apps/desktop`: 601
- `apps/web`: 7
- Rust: 347 (unchanged — no Rust changes in this migration)
- **Total: 1021 unit tests, all green**

---

## PR-G — audit + correctness pass (2026-07-20)

The close-of-PR-F entry above claimed the remaining work was "cleanup
only — no work needed for the engine itself." **That was wrong.** An
audit of the package against its own `REQUIREMENTS.md` found seven
defects, five of them runtime bugs, all of which the 66-test suite
passed straight through. Recorded here so the failure mode is not
repeated: *green gates did not mean a correct engine.*

### Why the tests missed all of it

`test/perf/budgets.test.ts` never imported package code. It defined a
`BUDGETS` object of literals and asserted the literals against
themselves (`expect(BUDGETS.ttff4kMs).toBe(800)`), plus twelve more
tests self-labelled `[vacuous]` asserting `toBeGreaterThan(0)`. Its
own header said each one would be "replaced with a real measurement"
in PR-D/E/F. PR-D, PR-E and PR-F all landed; the replacement never
happened. So REQUIREMENTS.md §3's "regression on any row fails the
build" and §5's "the budgets test asserts against the marks" were both
false for all twelve rows.

Separately, every cache fixture was `ImageBitmap`-shaped
(`width`/`height`), while the desktop preview actually writes
`VideoFrame`s (`codedWidth`/`codedHeight`). The type gap that caused
the NaN bug below was invisible to the fixtures.

### Fixed in this pass

1. **Decoded-frame memory cap was never implemented.** `FrameCache`'s
   `#memoryInsert` did `Map.set` with no cap, no eviction, and no
   `close()`. Frames accumulated one per seek for the lifetime of the
   process — and `dispose()` explicitly declined to clear them. The
   §3 "≤ 512 MB, LRU by GOP" row existed only as a constant in the
   vacuous test. Now enforced by `#evictMemoryUntilFits`, which closes
   every frame it evicts (per Chrome's WebCodecs guidance: a decoded
   frame holds a GPU surface the GC will not reclaim promptly).

2. **`estimateFrameBytes` returned `NaN` for every `VideoFrame`.** It
   read `frame.width * frame.height * 4`; `VideoFrame` has no
   `width`/`height`. The `NaN` propagated into `#stats.bytes` and into
   the IDB cap arithmetic, where `NaN > cap` is `false` — silently
   disabling *both* byte budgets. Now branches on `codedWidth`.

3. **IndexedDB LRU eviction threw 100% of the time.** `#evictUntilFits`
   opened `store.index('lastUsedUs')`, but `onupgradeneeded` never
   created that index. The guard `store.index ? … : …` was always
   truthy (`index` is a method), so it always took the throwing branch.
   Since `put` awaits eviction before writing, every over-cap write
   rejected. Fixed with a v1→v2 migration that creates the index, and
   a real `indexNames.contains` guard.

4. **Nothing was ever persisted from the desktop path.** `VideoFrame`
   is transferable but not structured-cloneable, so the IDB `put`
   rejected with `DataCloneError` — swallowed by a fire-and-forget
   `.catch` that logged a warning. The entire PR-E persistent cache
   was inert in the editor. Now `isPersistable()` skips the write
   honestly; the hot layer still serves the frame for the session.

5. **Frames bled between recordings.** `IndexedDBFrameStorage`
   assigned `#recordingId` and never read it, and `getFrameCache()` is
   a process-wide singleton keyed by bare timestamp. Open recording A,
   scrub to 5 s, open recording B, scrub to 5 s → **B painted A's
   frame.** Fixed with `FrameCache.setScope()` (closes and drops the
   previous source's frames) plus a per-recording IDB database name.

6. **`#size` never survived a reload.** Per-instance, initialised to 0,
   while the DB persists — so the cap was unenforceable until a single
   session happened to write 2 GB. Now recomputed at open.

7. **Contract violations.** ~30 `reject(new Error(…))` /
   `DOMException` sites and 6 `throw new Error(…)` replaced with
   `MediaError` + code, so consumers can branch on `.code` instead of
   string-matching (§5).

### Structural changes

- **`openMediaSource` / `seekTo` / `prefetchAround` were throw-stubs**
  ("not yet implemented — lands in PR-D") while the real implementation
  lived in `apps/desktop`. Since that code is pure Web-platform
  (Worker + WebCodecs + OffscreenCanvas, zero Tauri), it moved into
  `packages/media/src/playback/` and now backs the documented API.
  `apps/web` can use the editor preview engine for the first time.
- **MediaBunny is no longer re-exported from the main barrel.** A
  static re-export pulled all of MediaBunny into every consumer,
  defeating §5 tree-shaking and the §3 80 KB budget. Raw primitives
  now live on the `@recast/media/mediabunny` subpath, for worker
  modules only.
- **The package now type-checks.** `check` was `exit 0`; it is now a
  real `tsc --noEmit` against a new `tsconfig.json`, with an ambient
  declaration for untyped `gifenc`.
- **`budgets.test.ts` rewritten.** The 12 vacuous placeholders are
  replaced with 4 tests that exercise real cache code, plus an explicit
  list of the rows that are *not* enforceable in Node and why.

### Verified

- `@recast/media` 79, `apps/desktop` 595, `apps/web` 63,
  `@recast/captions` 59 — **796 green.**
- `pnpm --filter recast-desktop check` — 7857 files, 0 errors.
- `pnpm --filter recast-desktop ui:build` — green; the decode worker
  emits as its own chunk from inside the package
  (`_app/immutable/workers/worker-*.js`), confirming Vite resolves
  `new URL('./worker.ts', import.meta.url)` across the package boundary.
- The 7 new cache regression tests were each confirmed to **fail**
  against the pre-fix implementation before being kept.

## PR-H — gates, cancellation, dead code (2026-07-20)

Follow-up pass. Owner decisions: enforce bundle size in CI rather than build
a Playwright harness, relax the blanket AbortSignal rule to I/O-only, delete
the dead stubs.

### `sideEffects: false` was missing — the barrel was never tree-shakable

The new `test/perf/bundle.test.ts` (esbuild + gzip) caught this immediately.
Importing a single `MediaError` from the barrel cost **61.7 KB gz**; the same
import direct from `errors.ts` cost **0.1 KB**. Removing the MediaBunny
re-export in PR-G was necessary but not sufficient — without
`"sideEffects": false` a bundler must assume every module has side effects and
cannot drop any of them. Adding it took the barrel to **0.2 KB**.

| entry | before | after |
|---|---|---|
| barrel → `MediaError` | 61.7 KB | **0.2 KB** |
| barrel → `getFrameCache` | 64.1 KB | **2.7 KB** |
| `/playback` subpath | — | 3.3 KB |

### The web bundle row was measuring the wrong thing

§3 said "web (incl. tools) ≤ 150 KB"; the conversion surface measures 202 KB
and always would — `handlers` is an eager registry pulling MediaBunny, gifenc,
lamejs and fflate. But `apps/web/src/lib/tools/client.ts` imports **types
only** and spawns the worker lazily, so none of it blocks first paint. The row
conflated the page bundle with an on-demand worker chunk. Split into three
rows (page ≤ 5 KB, desktop ≤ 80 KB, conversion worker ≤ 220 KB), all gated.

### Other

- `AudioScheduler.load()` ran an unbounded fetch/decode loop with no
  cancellation; it now takes an `AbortSignal`, closes the `AudioContext` on
  abort, and rejects with `MediaError('cancelled')`. 3 tests added.
- §5's blanket "all exported async functions take an AbortSignal" is amended
  to I/O-or-unbounded-work only; `cacheStats`/`evictCache` are exempt.
- Deleted `sources.ts` (`encodeCanvasToMp4`, a throw-stub on the public API
  with zero callers). `sqlite-storage.ts` turned out to never have existed —
  the PR-E file map above listed a file that was never written.

### apps/desktop — `ipc.ts` split (first editor-extraction step)

`ipc.ts` was 1,739 lines mixing 83 type declarations with 116 `invoke()`
wrappers, so importing a shape dragged in the Tauri runtime. Types moved to
`ipc-types.ts` (806 lines), `ipc.ts` is now 1,030 and re-exports them for
backward compatibility. **26 files** — logic modules, panels, route logic and
tests — now import types without touching `invoke`, including `$lib/profiles`,
which resolves the ESM cycle its comment in `ipc.ts` was working around.

## PR-I — the engine the landing strip was standing in for (2026-07-20)

An audit found the preview painted **0 of 120 frames** during simulated
playback. PR-D shipped as production and the "later PRs" its header deferred to
never landed. This is those PRs.

### Playback (the P0)

- **Cache lookup was exact-match.** `readMemory` is `Map.get(tsUs)`, but frame
  timestamps land on presentation times while the render loop asks for whatever
  microsecond the rAF fell on — so it missed every time. Added
  `FrameCache.readNearest(tsUs, floorUs)`: newest frame at or before `tsUs`,
  never older than the segment floor, over a binary-searched sorted key index.
  The `floorUs` check is the part that stops the picture stepping back into cut
  content — the old code accepted the argument and did `void floorUs`.
- **Every rAF issued a seek that aborted the previous one.** Once decode cost
  more than a frame interval nothing ever completed. The worker now runs a
  `sink.canvases(startSec)` stream: `seek` starts a run, `playhead` only
  releases backpressure (parked past `LOOKAHEAD_SEC = 0.75`). One seek per
  jump instead of 60/second.
- **Frames carry their real PTS**, not the requested time, so the cache keys
  match what the reader looks up.
- **Late frames are kept.** They used to be dropped on a `seq` mismatch; a
  frame that arrives a tick late is still a valid picture for its own timestamp.

Measured on the streaming harness (`test/playback-realtime.test.ts`):

| decode latency | before | after |
|---|---|---|
| 5 ms | 0/120 painted, 120 seeks | **119/120 painted, 1 seek** |
| 25 ms | 0/120 painted, 119 aborted | **118/120 painted, 1 seek** |

### Resources

- **`frameBudget` is wired back in.** It had zero importers, so the cache ran a
  flat 512 MB at every resolution — 2.7× the surface budget empirically known to
  stall a 4K decoder. Moved into `packages/media/src/cache/frame-budget.ts` and
  applied via new `frameCacheCapBytes(w, h)` on source creation.
- **The parallel `<video>` decode is gone.** It was mounted with `preload="auto"`
  and played by the transport, decoding the whole file alongside the worker, and
  was only ever paused on file unload. It now stays paused while `mbReady` and
  serves as a seek-only transport plus the fallback element. `scoutEl` no longer
  mounts on the MediaBunny path — all its readers sit behind `!mbReady`.
- **Paused time no longer reads the `<video>`**, since we keep it paused; the
  store owns time on this path.
- **`prefetch` posts its frame back** and dedupes by target. It used to
  `await sink.getCanvas()` and discard the result, ~120 times per cut.
- **`fps` is measured** via `computePacketStats` instead of hardcoded to 30.

### Telemetry & dead code

- `mediabunny_preview_perf` shipped `{0,0,0}` to PostHog under real resolution
  cohorts. `stats()` now reports decoded-frames/sec, the served/asked hit rate
  scaled to source fps, and worst lateness.
- Deleted, all verified to have zero non-test importers: `frame-index.ts`,
  `gop-byte-budget.ts`, `audio-worklet-processor.ts` (also non-functional — no
  `port.onmessage`, built an `AudioContext` inside `AudioWorkletGlobalScope`),
  and `packages/media/src/audio/scheduler.ts`.
- **The packaged audio scheduler was removed rather than adopted.** No app code
  imported it, and adopting it would have *regressed* audio: it merges all
  tracks into one buffer, which would break the per-track system/mic mute the
  shipped `AudioTimelineEngine` supports. The proven engine stays; promoting it
  into the package is the future path.
- `apps/desktop/src/lib/playback/audio-schedule.ts` is now a re-export shim over
  `@recast/media`; it held a byte-identical copy of the scheduling math.
- `isUnsupportedContainer` is finally called — known-bad containers reject
  before a worker spawns instead of being discovered by try/catch.

### Verified

86 media / 570 desktop / 63 web / 59 captions green; both type-checks 0 errors;
`ui:build` green with the streaming protocol present in the emitted worker
chunk. **Still owed: runtime confirmation in the real WebView** — see below.

## PR-J — A/V sync + protocol cleanup (2026-07-20)

The last open item from the audit. The picture clock is a `performance.now()`
integrator; audio schedules against `AudioContext.currentTime`. Two crystals,
no correction — they separate over a long take, and nothing measured it.

- `AudioTimelineEngine` now records the output-time ↔ audio-clock anchor at
  schedule time and exposes `positionOutputSec`.
- New pure `av-sync.ts` (`resolveAvSync`) decides when to act. **Audio is the
  master**: a one-off picture re-anchor is far less noticeable than a gap or
  pitch artefact. Threshold 60 ms, inside ITU-R BT.1359's ~45 ms lead / 125 ms
  lag detectability window, and loose enough that jitter doesn't cause constant
  re-anchoring. 10 unit tests.
- `VideoPreview` applies it in the draw loop and tracks worst drift, now
  reported as `max_av_drift_ms` on `mediabunny_preview_perf` — so §3's drift
  row finally has real data behind it.
- `av-drift.ts` stays for the legacy `<audio>` path; the two are not
  interchangeable and each says so.

Cleanup: removed the `cancel` protocol message (declared, half-handled, never
sent — run supersession and `dispose` cover it) and `#inFlightSeq` (written,
never read for a decision once late frames became keepable). Fixed a redundant
master-mute term in `AudioTimelineEngine.#applyGains` that made the per-track
mute check dead.

New tests: `FrameCache.readNearest` floor semantics and index integrity across
eviction/scope-change (6), seek-vs-playhead request policy (4), A/V sync
policy (10). One of them caught a floating-point boundary bug in its own
assertion — `10 + 0.06 - 10` exceeds `0.06` — fixed by anchoring at zero.

## PR-K — instrumentation + doc rot (2026-07-20)

- **`performance.measure` instrumentation** (REQUIREMENTS.md §5, unmet since
  PR-A). New `src/marks.ts` emits `recast-media:time-to-first-frame` and
  `recast-media:seek-latency` onto the DevTools Performance timeline — the two
  §3 latency rows that can't be gated in Node are now at least *observable*
  during manual testing. Self-limiting (clears past 200 entries), inert when
  the Performance API is missing, and swallows its own errors so instrumenting
  can never break playback. 4 tests.
- **Stopped requesting impossible persistence.** The source passed
  `persist: true` for every frame while `isPersistable` always rejected
  `VideoFrame` — a call site that lied. Now explicitly memory-only, with the
  reason. Also avoids what would have become a per-frame IndexedDB write under
  the streaming decoder.
- **Doc rot from PR-I/J deletions:** `architecture.md` still listed
  `frame-index.ts` in two reading paths (deleted), and both `REQUIREMENTS.md`
  and `AGENTS.md` cited `webcodecs-source.ts:22` — a file deleted three PRs
  ago — as the authority for the VideoFrame-ownership rule. Repointed at
  `cache/index.ts`'s `#evictMemoryUntilFits`, which is the real single close
  path. Refreshed the file map, which still listed `sqlite-storage.ts` (a file
  that never existed).
- **Deleted `PLAN.md`.** Fully executed through PR-F and superseded by this
  log; per the standing rule, plans go once done. Its risk register was
  harvested first — every row is either resolved or captured below.

## PR-L — the first runtime test failed, and it was the move (2026-07-20)

Owner opened a recording and got `MediaError: worker error` on every file, so
the engine silently fell back to `<video>` and **nothing from PR-I…PR-K was
ever exercised**.

- **Cause: the package spawned its own worker.** `new URL('./worker.ts',
  import.meta.url)` inside `packages/media` makes Vite emit an out-of-root
  `/@fs/…/packages/media/…` dev URL, and SvelteKit's default `server.fs.allow`
  (app dirs + `node_modules`) answers **403**. The script never loads, so
  `onerror` fires with an **empty** message — which the source reported as the
  useless string "worker error". Production bundled it correctly the whole
  time, so `ui:build`, `tsc` and 802 green tests all passed over a dev-only
  break. Confirmed by curling the URL, not by reasoning.
- **Fix (owner's design): the host app owns the spawn.** `create` now takes
  `{ createWorker }` and the package never constructs a `Worker`. The body is
  exported as `startMediabunnyWorker()` on the new
  `@recast/media/playback/worker` subpath; the app mounts it from
  `$lib/playback/mediabunny-worker.ts` and spawns via
  `$lib/playback/mediabunny.ts`. The worker's top-level URL is now in-root, and
  its import of the package body is allowed because the app entry puts it in
  Vite's module graph. **`vite.config.ts` is byte-identical to before** — the
  earlier `server.fs.allow` patch was reverted, since a fix every consuming app
  must copy into its bundler config isn't a fix.
- A factory, not a `Worker` instance, so `create` still rejects unsupported
  containers *before* spawning — there's a test pinning that.
- `worker.ts` no longer reads `self` at module scope (bound in
  `startMediabunnyWorker`), so it's importable by tooling and tests.
- **Unrelated bug in the same console log: the filmstrip worker transferred
  `Blob`s.** `postMessage(msg, [blob])` — a Blob is structured-cloneable but
  NOT transferable, so every tile and the storyboard threw and the message was
  dropped. Timeline thumbnails have been silently dead. Transfer lists removed.
- New `test/worker-resolution.test.ts` pins the inverse invariant: the package
  contains no `new Worker` / `new URL(…, import.meta.url)`, exports the body,
  and the worker bundles standalone.

**Verified:** 103 media / 580 desktop green, both type-checks clean, dev serves
the worker 200 with no config, production chunk unchanged in substance
(355,712 B vs 355,671 B, full streaming protocol present).

### PR-L.2 — the actual P0: the canvas pool

With the worker finally running, the picture still froze on ~frame 1 while
overlays animated — the exact symptom the PR-I streaming rewrite claimed to
have fixed.

- **Cause:** `new CanvasSink(track, { poolSize: 8 })`. MediaBunny **recycles
  pooled canvases round-robin**, and we `postMessage(..., [canvas])` every one
  of them, which **detaches** it. On the 9th frame the sink drew into a
  detached canvas, the run threw, and `#onMessage` merely `console.warn`ed.
  Eight frames at 60fps is 0.13 s — indistinguishable from "stuck on frame 1".
- **Pooling is incompatible with this design by construction.** A pool assumes
  the consumer is done with each canvas before the next yield; we *keep* every
  frame in a cache. Fix is to not pool.
- **Why every gate missed it:** the existing harness stubs the *worker*, so how
  the worker drives the sink was never exercised — the bug lived entirely in
  the untested seam. New `test/worker-decode-run.test.ts` drives the REAL
  worker against a `vi.mock('mediabunny')` enforcing the true canvas contract
  (ring-buffer reuse, detach-on-transfer). Confirmed to fail pre-fix delivering
  **exactly 8 frames**, matching `poolSize` numerically.
- **No more silent freezes:** `MediabunnyVideoSource.onError` added, the warn
  escalated to `console.error`, and `VideoPreview` now drops back to `<video>`
  when a run dies mid-playback instead of holding a still frame forever.

Follow-up worth taking: `VideoSampleSink` + `toVideoFrame()` would remove the
decode→canvas→`VideoFrame` round-trip, since the main thread re-wraps every
transferred canvas in a `VideoFrame` regardless.

## What's genuinely left

1. **No browser-based perf harness.** 5 rows (TTFF, scrub p95,
   frame-to-glass, INP, audio drift) cannot be measured in Node and
   remain documented targets, not gates. Deliberate: owner chose CI
   bundle gates + real-user telemetry over a Playwright harness, on the
   grounds that one CI runner's timings don't represent user hardware.
   The `mediabunny_preview_*` PostHog events are the intended source.
2. **Runtime verification is owed by the owner.** Every check is static or
   unit-level. Specifically unconfirmed in a real WebView: that playback now
   advances smoothly (PR-I's P0), that only one decode session runs, that
   crossing a cut is seamless, that opening two recordings back-to-back never
   shows the first one's frame, and that memory plateaus on a long 4K scrub.
3. **The IndexedDB layer is not used by the desktop preview.** A `VideoFrame`
   can't be structured-cloned, and the streaming decoder would attempt a write
   per frame — so the source now writes memory-only and re-decodes instead,
   which is cheap once the pipeline streams. The persistent layer is built,
   tested and correct; it's waiting on a consumer that stores `ImageBitmap`
   (the future in-browser editor, where re-fetching over the network is the
   expensive part). The "survives editor restarts" line in the End goal above
   is therefore **not met today** — deliberately.

4. **`filmstrip-*` remains out of scope** (REQUIREMENTS.md §1) and
   still composes MediaBunny directly, now via the `/mediabunny`
   subpath.
5. **The editor package itself.** `ipc.ts` is split; the remaining work
   is the 2,484-line `routes/editor/[file]/+page.svelte` shell and the
   `<Editor />` prop boundary. ~89% of the editor's 32.7k lines are
   already Tauri-free.

---

## How to verify (for a future reviewer)

```bash
# 1. Static type-check + Svelte
pnpm check

# 2. Unit tests
pnpm test:media          # @recast/media: 66
pnpm test:web            # apps/web: 7
pnpm test:desktop        # apps/desktop: 601 (includes the playback surface test)

# 3. Rust
pnpm test:rust

# 4. Optional: production build
pnpm --filter recast-desktop build
```

After step 1 + 2, the migration is verified for behavior; step 4
verifies bundle size / signing.

---

## File map (current, close of PR-J)

```
packages/media/
├── src/
│   ├── audio/schedule.ts               # keptRegions + planAudioSchedule (shared)
│   ├── cache/
│   │   ├── index.ts                    # FrameCache: LRU + readNearest + scope
│   │   ├── frame-budget.ts             # resolution-adaptive caps
│   │   ├── indexeddb-storage.ts        # persistent layer (v2 schema)
│   │   ├── storage.ts                  # FrameStorage + CachedFrame/isPersistable
│   │   └── unsupported-formats.ts      # gap list, gates create()
│   ├── playback/
│   │   ├── source.ts                   # MediabunnyVideoSource (main thread)
│   │   ├── worker.ts                   # streaming decode run; host app mounts it
│   │   └── index.ts                    # /playback subpath
│   ├── marks.ts                        # performance.measure instrumentation
│   ├── mediabunny.ts                   # /mediabunny subpath (workers only)
│   ├── conversion.ts, encoders.ts, handlers.ts, input.ts, protocol.ts
│   ├── errors.ts, playback.ts, seek.ts, vendor.d.ts
│   └── index.ts                        # tree-shakable barrel
└── test/                               # 96 tests
    ├── playback-realtime.test.ts       # 60fps playback + seek policy
    ├── cache.test.ts                   # lifetime, caps, readNearest
    ├── perf/{budgets,bundle}.test.ts   # enforced budget rows
    └── audio, conversion, seek, smoke, marks, frame-budget, unsupported-formats

apps/desktop/src/lib/playback/          # what remains desktop-side
├── audio-engine.ts                     # Web Audio timeline engine (per-track gain)
├── audio-schedule.ts                   # re-export shim over @recast/media
├── mediabunny.ts                       # spawns the decode worker (app owns this)
├── mediabunny-worker.ts                # worker entry: startMediabunnyWorker()
├── av-sync.ts                          # picture-follows-audio policy (new)
├── av-drift.ts                         # legacy <audio> path only
└── clock.ts                            # PlaybackClock
```
