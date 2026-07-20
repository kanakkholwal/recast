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

- `@recast/media` 71, `apps/desktop` 595, `apps/web` 63,
  `@recast/captions` 59 — **788 green.**
- `pnpm --filter recast-desktop check` — 7857 files, 0 errors.
- `pnpm --filter recast-desktop ui:build` — green; the decode worker
  emits as its own chunk from inside the package
  (`_app/immutable/workers/worker-*.js`), confirming Vite resolves
  `new URL('./worker.ts', import.meta.url)` across the package boundary.
- The 7 new cache regression tests were each confirmed to **fail**
  against the pre-fix implementation before being kept.

## What's genuinely left

1. **No browser-based perf harness.** 8 of the 12 §3 budget rows
   (TTFF, scrub p95, frame-to-glass, INP, bundle size, audio drift)
   cannot be measured in Node. They are documented targets, not gates,
   until a Playwright + fixture-recording harness exists. This is the
   single biggest remaining gap and it is now stated in the test file
   rather than implied away.
2. **`AbortSignal` coverage is partial.** §5 says every exported async
   function takes one; `openInput`, `inputContainerKind`,
   `createAudioScheduler`, `evictCache`, `cacheStats` and the
   `AudioScheduler` methods still don't. `openMediaSource` and
   `seekTo` now do.
3. **Runtime verification is owed by the owner.** Every check above is
   static or unit-level. Nobody has opened two recordings back-to-back
   and confirmed fix #5 visually, or watched memory plateau at 512 MB
   during a long scrub session. See "How to verify" below.
4. **`sqlite-storage.ts` is still a stub** and `encodeCanvasToMp4`
   still throws — both are unreferenced. Delete or implement.
5. **`filmstrip-*` remains out of scope** (REQUIREMENTS.md §1) and
   still composes MediaBunny directly, now via the `/mediabunny`
   subpath.

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

## File map of what changed (close of PR-F)

```
packages/media/                          # NEW workspace package
├── src/
│   ├── audio/
│   │   ├── schedule.ts                 # keptRegions + planAudioSchedule
│   │   └── scheduler.ts                # AudioScheduler interface + 2 impls
│   ├── cache/
│   │   ├── index.ts                    # FrameCache orchestrator
│   │   ├── indexeddb-storage.ts        # default backend
│   │   ├── sqlite-storage.ts           # stub (kept for forward compat)
│   │   ├── storage.ts                  # FrameStorage interface
│   │   └── unsupported-formats.ts        # gap list (AVI / FLV / WMV / RealVideo / 3GP)
│   ├── conversion.ts                    # runConversion + helpers
│   ├── encoders.ts                     # GIF / WAV / MP3 / ZIP
│   ├── errors.ts                       # MediaError + MediaErrorCode
│   ├── handlers.ts                     # trim / mute / compress / resize / etc.
│   ├── input.ts                        # openInput
│   ├── playback.ts                     # PlaybackSource interface + openMediaSource
│   ├── protocol.ts                     # ToolOp, ConvertError, etc.
│   ├── seek.ts                         # snapToSeekTarget + nextCutWithin
│   ├── sources.ts                      # encodeCanvasToMp4
│   └── index.ts                        # public API barrel
└── test/
    ├── audio.test.ts
    ├── cache.test.ts
    ├── conversion.test.ts
    ├── seek.test.ts
    ├── smoke.test.ts
    ├── unsupported-formats.test.ts
    └── perf/budgets.test.ts

apps/desktop/src/lib/playback/         # REMAINING
├── mediabunny-{source,worker,test}.ts  # MediaBunny-backed playback
├── mediabunny-source.test.ts           # 6 tests, Worker mock
├── audio-engine.ts                     # per-track gain engine (mirrors Worklet impl)
├── audio-schedule.ts                   # (legacy; superseded by @recast/media/audio/schedule)
└── frame-{index,budget}.ts             # (legacy; kept, no longer used by preview)

apps/desktop/src/lib/playback/         # DELETED in PR-F
├── webcodecs-{source,worker,protocol}.ts
├── mp4-{demux,sample-table,sample-table.test}.ts
├── feature-flag.{ts,test.ts}
└── gop-byte-budget.{ts,test.ts}        # (also deleted, no longer needed)

apps/desktop/src/components/editor/    # CHANGED
├── VideoPreview.svelte                 # uses MediabunnyVideoSource only
├── properity-panel/AudioPanel.svelte   # per-track mute + volume
├── properity-panel/captions-panel...   # cut-aware caption clipping (already)
└── _components/CaptionOverlay.svelte   # cut-aware (already)

apps/desktop/src/__tests__/            # NEW (PR-D/E/F)
├── cut-jump-parity.test.ts              # 14 tests
├── editor-pipeline.test.ts              # 12 tests
└── unsupported-formats.test.ts          # 6 tests

apps/desktop/src/lib/timeline/         # CHANGED (PR-E)
├── filmstrip-source.ts                 # MediabunnyTileProvider
└── filmstrip-worker.ts                 # MediaBunny-backed worker

packages/analytics/src/                # CHANGED (PR-F)
└── taxonomy.ts                          # webcodecs_preview_* → mediabunny_preview_*
```
