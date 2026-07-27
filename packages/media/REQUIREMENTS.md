# `@recast/media` Requirements Contract

The non-negotiable contract every consumer of `@recast/media` can rely on,
and every contributor to this package must respect.

If a rule here is in tension with code, the rule wins — open a PR that
fixes the code, not the rule.

---

## 1. Purpose & scope

`@recast/media` is the shared media-processing layer for Recast.

**Consumers:**

- `apps/desktop/src/routes/editor/[file]/` — primary target: video editor
  preview pipeline.
- `apps/web/src/routes/tools/*` — browser conversion tools (trim, mute,
  compress, resize, transcode, extract-audio, video-to-gif, audio-to-mp3,
  extract-frames).
- **Future:** an in-browser video editor for 100 GB+ source files. The
  design headroom is built in now (range-fetched `StreamSource`, byte-
  budgeted IndexedDB cache, streaming decode-ahead).

**Out of scope:**

- The Rust export pipeline (`apps/desktop/src-tauri/src/commands/export/*`).
- The web app's screenshot editor (`packages/application/src/screenshot-editor/*`);
  it keeps its `mp4-muxer` legacy path.
- `apps/desktop/src/lib/timeline/filmstrip-*` (separate hot path; revisit later).
- Desktop `/tools` route mirroring apps/web's `/tools`.

---

## 2. Public API surface

Consumers import ONLY from `@recast/media`. Direct imports from `mediabunny`
are forbidden outside this package (enforced by Biome lint + a CI grep check).

```ts
// input + conversion
export { openInput, runConversion, outputFormatFor, inputContainerKind };
export type { ConversionParams, ContainerKind };

// worker-bridged playback (the new surface)
export { openMediaSource, seekTo, prefetchAround, evictCache, cacheStats };
export { frameBudget, frameCacheCapBytes };
export type { PlaybackSource, PlaybackEvent };

// audio scheduling math (shared with the desktop engine)
export { keptRegions, planAudioSchedule };
export type { Region, ScheduledChunk };

// conversion tools' shared protocol (apps/web)
export { ConvertError };
export type { ConvertHandler, JobContext, HandlerResult };

// error surface
export { MediaError };
export type { MediaErrorCode };
```

`MediaErrorCode` is the closed union
`{ unsupported, bad-input, decode-failed, worker-died, cancelled, internal, too-large }`.

Two additional subpaths exist, both deliberately off the main barrel:

```ts
// @recast/media/playback — the decode worker's main-thread proxy. Lazily
// imported by `openMediaSource`; import directly only when you need the
// lower-level sync `frameAt` surface (the editor's rAF loop does).
export { MediabunnyVideoSource };
export type { MediabunnySourceOptions };

// @recast/media/playback/worker — the decode worker BODY. The host app mounts
// it from a worker entry file of its own, and passes `createWorker` to
// `create`. This package must never spawn the worker: the resulting URL points
// outside the app's root and its dev server then has to whitelist the path.
export { startMediabunnyWorker };

// @recast/media/mediabunny — raw MediaBunny primitives, worker modules only.
export { ALL_FORMATS, BlobSource, CanvasSink, Input, UrlSource };
```

---

## 3. Performance budget (non-negotiable, merge-blocking)

Regression on any row fails the build via
`packages/media/test/perf/budgets.test.ts`.

| Concern | Budget | Notes |
|---|---|---|
| Time-to-first-frame | ≤ 800 ms (4K @ 60 fps), ≤ 200 ms (1080p @ 30 fps) | `load_editor_document` return → first paint of `VideoPreview` |
| Scrub seek (cached frame) | ≤ 50 ms p95 | Hits decoded-frame cache |
| Scrub seek (cold frame) | ≤ 200 ms p95 | Single GOP decode |
| Frame-to-glass during playback | ≤ 16.7 ms p95 (60 fps) | Whole pipeline: decode → composite → upload → display |
| Cut-cross latency | ≤ 250 ms p95 | Existing baseline; non-regression budget |
| INP during playback | ≤ 100 ms | Scrub / cut / split must not block input |
| Decoded-frame memory | resolution-adaptive via `frameCacheCapBytes`, ≤ 512 MB | A flat cap safe at 1080p starves the decoder's surface pool at 4K |
| IndexedDB cache | ≤ 2 GB hard cap (user-configurable in Settings; default 2 GB), LRU by recency × bytes | Re-scrub reuse |
| `@recast/media` bundle — desktop | ≤ 80 KB gz | Editor preview surface: cache + errors + playback subpath |
| `@recast/media` bundle — web page | ≤ 5 KB gz | `tools/client.ts` is types-only and spawns the worker lazily; nothing here blocks first paint |
| `@recast/media` bundle — conversion worker | ≤ 220 KB gz | On-demand chunk, fetched only after the user starts a conversion. Pulls MediaBunny + gifenc + lamejs + fflate |
| Package is side-effect-free | `sideEffects: false` | Without it a lone `MediaError` import cost 61 KB gz instead of 0.2 |
| Worker isolation | decode + demux in Worker | Main thread never touches `VideoDecoder` |
| Audio/video sync drift | ≤ 1 audio frame (~10 ms @ 48 kHz) over 10 min | Web Audio scheduling in the desktop `AudioTimelineEngine`; open-loop, no continuous correction |

---

## 4. Curated web.dev guides (read before opening a PR)

Every contributor to this package and the editor's preview pipeline must
read the relevant subset below before opening a PR. One-line takeaway per
article.

### Real-time rendering / compositing

- https://web.dev/articles/offscreen-canvas — Canvas in a Worker via
  `transferControlToOffscreen`. **Anchor doc.** Compositor-on-worker when
  main-thread pressure is measured.
- https://web.dev/articles/rendering-performance — Avoid layout thrash;
  prefer transforms; `will-change`; layer promotion.
- https://web.dev/articles/animations-guide — `requestAnimationFrame`
  scheduling; frame-perfect cadence.
- https://web.dev/articles/animations-api — WAAPI for non-timeline UI
  motion (not the playback loop).
- https://web.dev/articles/canvas-performance — GPU-accelerated 2D; batch
  state changes; avoid `shadowBlur`.

### Workers & parallel execution

- https://web.dev/articles/workers-overview — Worker topology choices
  (dedicated / shared / service).
- https://web.dev/articles/off-main-thread — Moving expensive work off the
  main thread. **Anchor doc** for our decode/composite split.
- https://web.dev/articles/module-workers — Modern syntax for workers
  (Tauri's webview supports these).
- https://web.dev/articles/broadcast-channel — Cross-tab sync (library →
  editor hand-off; future PiP).
- https://web.dev/articles/two-way-communication-with-service-workers —
  For offline asset caching.

### WebCodecs / media

- https://web.dev/explore/media — Hub; latest WebCodecs + Web Audio guidance.
- https://developer.mozilla.org/en-US/docs/Web/API/WebCodecs_API — Canonical
  reference (MediaBunny sits on top).
- https://developer.mozilla.org/en-US/docs/Web/API/VideoFrame —
  Transferable, refcount, close semantics. **Anchor doc** for any code
  that touches decoded frames.

### Caching / large-file streaming

- https://web.dev/articles/offline-cookbook — Caching strategies.
- https://web.dev/articles/love-your-cache — HTTP cache fundamentals.
- https://web.dev/articles/http-cache — When the browser can avoid a round-trip.
- https://web.dev/articles/service-workers-cache-storage — Service-worker-
  mediated fetch for `StreamSource`.
- https://web.dev/articles/cache-api-quick-guide — Cache API for codec config
  / moov box caching.
- https://web.dev/articles/indexeddb-best-practices — IndexedDB hygiene.
  **Anchor doc** for the decoded-frame cache.

### Input responsiveness

- https://web.dev/articles/optimize-input-delay — Long tasks kill scrub
  responsiveness.
- https://web.dev/articles/inp — Interaction-to-Next-Paint target (≤ 200 ms).
- https://web.dev/articles/event-loop — Slicing long tasks with
  `scheduler.yield()`.

### Memory & lifecycle

- https://web.dev/articles/memory-management — LRU cache hygiene, GC
  pressure with large buffers.
- https://web.dev/articles/when-not-to-use-an-anti-pattern — Explicit
  `clearInterval` / `dispose()` patterns.
- https://web.dev/articles/reduce-javascript-payloads-with-tree-shaking —
  Keep `@recast/media` bundle small.

### Observability & debugging

- https://web.dev/articles/measure — Measure before optimizing; WebCodecs
  devtools.
- https://developer.mozilla.org/en-US/docs/Web/API/Performance_API —
  `performance.mark` / `measure` hooks.
- https://web.dev/articles/rail — RAIL model; "Animation" budget is the rubric.

### UI thread & async patterns

- https://web.dev/articles/async-functions — Decode / cut-cross paths are
  all `await`.
- https://web.dev/articles/promises — Backpressure signals from MediaBunny's
  pipeline.
- https://web.dev/articles/using-promises — Error propagation; no silent
  swallow.

### Audio

- https://developer.mozilla.org/en-US/docs/Web/API/AudioWorklet — Sample-
  accurate scheduling that survives jank. Relevant if the desktop
  `AudioTimelineEngine` is ever promoted into this package.

---

## 5. Implementation rules

- **No `mediabunny` import outside `packages/media`.** Direct imports are
  forbidden in consumer code.
- **Every exported async function that does I/O or unbounded work takes an
  `AbortSignal`.** Fetch/decode loops, worker round-trips and storage reads
  MUST be cancellable and MUST release partial work (close the `AudioContext`,
  dispose the worker) before rejecting with `MediaError('cancelled')`. Fast
  local reads (`cacheStats`, `evictCache`) are exempt — a signal there is
  ceremony, not safety. Amended 2026-07-20 from a blanket "all async exports"
  rule that the code had never satisfied.
- **All cancellable operations resolve only after resources are released.**
  No leaked `VideoFrame`s, `AudioBuffer`s, or `OffscreenCanvas`s.
- **`VideoFrame` ownership** crosses the worker boundary to the consumer;
  the producer side MUST NOT close a frame until the consumer returns a
  release message. This is the same invariant as the current
  `packages/media/src/cache/index.ts` (`#evictMemoryUntilFits`), which is the
  single close path for every decoded frame.
- **Workers are module workers** (`new Worker(url, { type: 'module' })`).
- **IndexedDB schema is versioned** via `onupgradeneeded`; migrations are
  explicit, not best-effort.
- **Errors throw `MediaError`** with `code` from §2. No
  `throw new Error("…")` in this package.
- **`performance.mark` / `measure` at every stage boundary.** The budgets
  test asserts against the marks.
- **Backpressure is propagated via promises.** `await mediaSource.add(...)`
  style; never spin.
- **Bundle is tree-shakable.** Each export is its own module; consumers
  import only what they touch.
- **The Rust export pipeline stays untouched.** `EnqueueExportRequest` IPC
  payload is byte-stable.
- **Raw MediaBunny primitives are exported ONLY from the
  `@recast/media/mediabunny` subpath**, never the main barrel — a static
  re-export on the barrel pulls the whole library into every consumer and
  breaks both the tree-shaking rule above and the §3 bundle budgets. Worker
  modules are the only sanctioned consumers of that subpath.
- **A test that asserts a constant against itself is not a test.** Every
  assertion in `test/perf/budgets.test.ts` must import and exercise package
  code. If a budget row cannot be measured in Node, list it in the
  "not enforced here" comment with the reason — do not write a placeholder
  that passes vacuously. (The original file did exactly that for all twelve
  rows and hid five runtime bugs for three PRs; see MIGRATION-LOG.md PR-G.)

---

## 6. Browser API surface

| API | Used for | Notes |
|---|---|---|
| WebCodecs (`VideoDecoder`, `VideoFrame`) | Inside MediaBunny | We don't touch directly; MediaBunny wraps it |
| OffscreenCanvas | Compositor (optional) | `transferControlToOffscreen` migrates if main-thread pressure measured |
| Web Workers (module) | Decode + demux | Worker-from-day-one |
| AudioWorklet | Sample-accurate audio | Replaces `apps/desktop/src/lib/playback/audio-engine.ts` (fallback kept until testing) |
| IndexedDB | Decoded-frame cache | ≤ 2 GB LRU by recency × bytes |
| Streams API (`ReadableStream`, `WritableStream`) | MediaBunny `StreamSource` | |
| Fetch with Range | Web `StreamSource` against API | |
| Tauri `asset:` protocol | Desktop `StreamSource` | Already in use today |
| `requestAnimationFrame` | Editor playback loop | Main thread, as today |
| `scheduler.yield()` | Slicing > 50 ms tasks | Chromium ≥ 129 (Tauri supports) |
| BroadcastChannel | Cross-tab hand-off (future) | |
| `URL.createObjectURL(Blob)` | `BlobSource` plumbing | Idiomatic |
| `performance.mark` / `measure` | Perf-budget instrumentation | Required |
| Compression Streams API | At-rest decoded-frame cache (future) | TBD |

---

## 7. Testing contract

- **Vitest** for unit logic (decode scheduling, cache eviction, time-map math).
- **`packages/media/test/perf/budgets.test.ts`** — asserts every row of §3.
  Fails the build on regression.
- **`packages/media/test/perf/cut-jump.test.ts`** — cut-jump parity fixture.
  Required to be green before PR-F deletes legacy files.
- **Tauri build smoke** — `pnpm tauri build` green on every PR that touches
  the editor preview.

---

## 8. Future: 100 GB+ browser editor (design headroom built in now)

- IndexedDB cache; LRU; byte-budgeted (default 2 GB; user-configurable in
  Settings).
- Range-request adapter behind the same `StreamSource` interface Tauri
  already speaks.
- Compression Streams API for at-rest decoded-frame cache (TBD).
- BroadcastChannel for cross-tab "open in editor" hand-off.
- `BlobSource` + streaming range fetches for large file playback in the
  browser without ever loading the whole file into memory.
- Squeeze-eval-check: every PR that changes the public API surface must
  pass `pnpm build` and report bundle sizes in the PR description so we
  notice budget drift early.
