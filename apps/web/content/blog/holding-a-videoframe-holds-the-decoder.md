---
kind: post
title: "Holding a VideoFrame holds the decoder"
description: "A 4K recording played for two seconds and froze. Then it crashed on open. Then exports were slow, but only sometimes. Every symptom hid a different wrong assumption: that a decoded frame is just a picture, that a slow load needs a faster engine, that variance is noise, and every real cause was found by measuring, not guessing."
slug: holding-a-videoframe-holds-the-decoder
date: 2026-07-21
author: Kanak
tags: [engineering, desktop, webcodecs, mediabunny, performance, video, tauri]
published: false
---

A 4K recording would play for a second or two, freeze, and stay frozen. Dragging the scrubber somewhere else bought another second or two, then it froze again. Smaller recordings were fine.

That symptom turned out to be five unrelated bugs stacked on top of each other, and the ones that mattered came from the same wrong assumption. It is an easy assumption to carry into a WebCodecs pipeline without noticing: that a decoded frame is a picture, and that keeping a few around is a caching decision like any other.

It is not. A decoded frame is a piece of the decoder, and so, it turns out, is a decode run.

## The three boring ones first

Worth listing because each was found by measuring rather than reasoning, and each looked like the whole problem while it lasted.

**The engine was not running at all.** The decode worker had moved into a shared workspace package. In a Vite dev server, a worker in a linked package resolves to an out-of-root `/@fs/` URL, and SvelteKit's default `server.fs.allow` returns 403 for it. The worker script never loaded. A worker that fails to load fires `onerror` with an empty message, so what we logged was the useless string `worker error`, and the preview quietly fell back to a plain `<video>` element. Production bundled it correctly the whole time, so the build was green, the tests were green, and playback was dead in the app we actually run.

The fix was not to widen the dev server's allow list. That would be a fix every consuming app has to copy into its own bundler config. The package now exports the worker body and the host app spawns it, so the worker URL always resolves against the app's own root.

**The decode run died a few frames in.** MediaBunny's `CanvasSink` takes a `poolSize`, and the docs are clear about what it does: pooled canvases "will be reused in a ring buffer / round-robin type fashion." We had set it to 8, and we were also transferring each canvas to the main thread with `postMessage(msg, [canvas])`. Transferring an `OffscreenCanvas` detaches it. On the ninth frame the sink drew into a detached canvas and the run threw. Eight frames at 60fps is 0.13 seconds, which is visually indistinguishable from a still image.

Pooling assumes the consumer is finished with each canvas before the next one arrives. We were keeping every frame. The two models are incompatible, and the fix was to stop pooling.

**The file was being rewritten underneath the reader.** Recast projects are zip bundles, and opening one extracts the video to a cache directory. The extraction used a bare `File::create`, which truncates the target to zero instantly and then takes several seconds to write 637MB back. Any reader in that window sees the `ftyp` box, so `canRead()` succeeds, but the `moov` box is not there yet, so the file appears to contain no video track. Both the preview worker and the filmstrip worker failed identically, which is what made it look like a decoder problem rather than a file problem.

Proving it was a race rather than a corrupt file took about a minute: the same file parsed perfectly through MediaBunny's `FilePathSource` in Node, one video track, avc, 1920x1080, 230 seconds. A file that parses fine now and had no track thirty seconds ago is being written while it is read. Small recordings extract fast enough to hide the window entirely, which is why only large ones failed.

Extraction now skips files that are already the right size and publishes through a temporary name and an atomic rename.

## The one that mattered

With all of that fixed, 4K played for one to two seconds and froze. Scrubbing produced another one to two seconds.

That shape is specific. A leak degrades gradually. This was a hard stop, followed by full recovery from an action that creates a new decoder. Whatever was exhausted was per-decoder, and a scrub was allocating a fresh one.

A decoded `VideoFrame` is not a copy of a picture. It is a handle to one of the decoder's output surfaces, and decoders have a small fixed number of them. Hold enough and the decoder has nowhere to write, so it accepts input and emits nothing. It does not error. It just stops.

We were holding them in three places at once:

- MediaBunny's sample generator keeps up to 8 decoded samples when its queue is non-empty. That is in its source: `computeMaxQueueSize` returns 8 as soon as any decoded sample is outstanding.
- Our frame cache held 4 at 4K.
- One more was being rendered.

Roughly 13 surfaces, at about 33MB each, so around 430MB of decoder-owned memory. Our own budget module thought it was enforcing a 192MB ceiling, and it was doing that job correctly. It just had no idea the other 8 existed.

That is the part worth generalising. The budget was not wrong about its own numbers. It was wrong about its scope, and a budget that only counts the allocations you made yourself will read as healthy right up until the resource runs out.

## The numbers that make it concrete

Paul Adenot's WebCodecs performance talk from the [W3C Media Production Workshop](https://www.w3.org/2021/03/media-production-workshop/talks/paul-adenot-webcodecs-performance.html) is the best single source on why frame memory dominates this kind of pipeline. Two things from it shaped what we built.

First, the size of the problem. A 4K YUV420 frame is about 16MB. Copying one costs 6.6ms with a hot cache and 17ms cold. The frame budget at 60Hz is 16.6ms. A single uncached copy of a single frame can consume the entire budget, and HDR is worse: he measures a Full HD P010 frame at 32MB, 15ms hot and 33ms cold.

Second, the direction of travel. The talk notes that "there is currently no way to become the owner of the memory behind a VideoFrame," and that GPU-to-CPU readbacks and CPU-to-GPU texture uploads are both expensive. The API deliberately makes copies visible through `copyTo()` so you have to think about them.

So the goal is not to avoid ever copying a frame. It is to copy it at most once, into something you own, and give the decoder its surface back immediately.

## What the engine looks like now

Decode runs in a worker, driven by MediaBunny's `VideoSampleSink`. Frames come back as `VideoFrame` objects transferred across the thread boundary, so there is no structured-clone copy on the way.

On arrival, each frame is uploaded into a texture from a ring we allocate ourselves, and then closed in the same tick. The close happens in a `finally`, so a consumer that throws mid-upload still cannot leak a surface. From the decoder's point of view, a frame is borrowed for the length of one `texImage2D` call and then handed straight back.

The renderer no longer receives frames at all. Each animation frame it asks the ring which texture corresponds to the current playhead and binds it. That selection is a pure function, which means the part most likely to be subtly wrong is the part that is easiest to test:

```ts
export function pickSlot(slots: readonly RingSlot[], tUs: number, floorUs: number): number
```

The floor argument is load-bearing. Frames earlier than the current segment's start belong to a region the user cut out, so returning one steps the picture backwards into deleted content at every cut boundary. Both the "at or before the playhead" rule and the floor have tests, because both have been broken before.

The payoff is that buffer depth stopped being a decoder question. Textures are ordinary GPU memory, so how many frames we buffer is now a memory budget we control: about 7 frames at 4K, 16 at 1080p, against a 256MB ceiling. Kapwing's editor makes a similar trade, keeping decoded frames in memory and closing the ones it does not need, and the browser-based editor writeup by [Alexsandro Souza](https://www.linkedin.com/pulse/building-browser-based-video-editor-modern-web-alexsandro-souza-9vyde/) lands on a 30-frame cache at roughly 240MB for 1080p, which is close enough to our budget to be reassuring.

## The cache that was making things worse

One more bug fell out of this, and it is a good argument for testing behaviour rather than implementation.

The frame cache evicted least-recently-used. That is the default answer to "which one should I drop," and it is exactly wrong for sequential playback. Frames decoded ahead of the playhead have never been read, so they are permanently the least recently used entries. Every time a new frame arrived, the cache dropped the one the playhead was about to reach, and the decoder then decoded it again.

A test harness that plays 120 frames and counts how many distinct pictures actually got painted caught this. Before the lookahead and eviction fixes it painted 6 distinct frames in two seconds. After bounding decode-ahead to the budget it painted 16. After replacing LRU with distance-from-playhead it painted 56.

Sixteen distinct frames in two seconds is eight updates per second, which is close to the mysterious "roughly 8fps" stall this codebase had been chasing on and off for months.

## Then scrubbing froze it, for the same reason in a different disguise

Playback was clean after all that. Dragging the scrubber froze the picture until
the next drag.

The run trace made it obvious in a way that no amount of reading the code had:

```text
run 564 from 229.747s ... run 660 from 41.044s
run 132 ended after 0 frames (superseded)
run 129 ended after 0 frames (superseded)
```

Roughly a hundred decode runs created during one drag, and runs numbered in the
low hundreds only finishing during that burst, minutes after they started.

Every `seek` starts a new sample generator, and every generator brings its own
`VideoDecoder`. Our supersede check happens at the top of the loop body, so a
run only notices it has been replaced after its next sample arrives. A run that
has not yet produced its first frame is parked inside `for await` with nothing
to check, so it cannot exit, and it cannot reach the `finally` that closes its
decoder. One pointer drag left about a hundred decoders alive.

Same failure as before, one level up. We were not holding frames this time, we
were holding decoders, and the pool ran out exactly the same way.

The fix has two halves. The worker keeps a handle to the live generator and
calls `return()` on it before starting the next run. MediaBunny's `return()` is
a hand-rolled iterator method that marks the run terminated and closes the
decoder straight away, so it does not matter that the old run is still waiting
for a sample that may never come. Separately, the main thread now rate limits
seeks to one per 50ms with the newest target winning, so a drag stops rebuilding
a decoder per pointer move.

Cut jumps needed checking before shipping that limiter, because a 50ms delay at
every cut would be a bad trade. They are unaffected: steady playback sends a
`playhead` message rather than a `seek`, so a jump always has a quiet window
behind it and fires immediately. Only jumps arriving back to back, which is
exactly a drag, get collapsed.

## Audio was ahead the whole time

While reading around this, one line in the web.dev article on [audio output latency](https://web.dev/articles/audio-output-latency) described our own bug back to us: use `outputLatency` to work out "when a given audio timestamp is reaching the user's ears and then properly paint video frames to match that."

Our preview uses audio as the master clock, because a one-off correction to the picture is far less noticeable than a gap or a pitch artefact in sound. The clock we were using was `AudioContext.currentTime`, which is when a sample is handed to the audio hardware, not when it reaches anyone. On Bluetooth that gap is commonly 150 to 300ms. Our resync threshold is 60ms, chosen to sit inside the roughly 45ms lead and 125ms lag where ITU-R BT.1359 puts detectability. So on any wireless or USB audio device, the correction we were applying was smaller than the error we were not measuring.

The position the listener is actually hearing is now computed by subtracting the reported output latency, falling back to `baseLatency` where `outputLatency` is unavailable.

## What we deliberately did not copy

The [CapCut case study](https://web.dev/case-studies/capcut) is the most cited piece of writing on browser video editing, and almost none of it applies to us. Their central decision was compiling an existing C++ editing engine to WebAssembly with Emscripten, which let them share code between their native and web apps and gave them a roughly 300% speedup from SIMD on effect rendering. That is the right call when you already own a C++ engine and need it in a browser. Recast runs native Rust and FFmpeg through Tauri, so a WebAssembly layer would be strictly slower and more complex for the same result.

The same goes for the encode half of the canonical WebCodecs pipeline. The usual diagram runs decode, process, encode, mux entirely in the browser. We do the left half and stop. Export is native FFmpeg, which gets us hardware encoders, the full filter graph, subtitle rendering through libass, and no in-memory `ArrayBuffer` ceiling to run into on a long 4K export.

There is a real cost to that split and it is worth being honest about it. In the browser-only pipeline, the same render code sits between decode and encode, so the preview and the export are the same renderer by construction. Ours are two implementations, a WebGL2 shader and an FFmpeg filter graph, which have to be kept in agreement by hand. Every visual feature is a parity risk, and that is the price we pay for a native export.

## Measuring the thing we were most likely to be wrong about

The design uploads every decoded frame into a texture, and that upload was the
part I was least sure of. If the decoded frame stays GPU-resident, it is a cheap
blit. If it is CPU-backed, Adenot's numbers put it at 6 to 17ms at 4K, on the
main thread, sixty times a second. Reasoning could not settle it, so the ring
times every upload and reports average, worst case, and a warning the first time
one crosses half a frame budget.

The answer, on a real recording:

```text
[ring] 13800 uploads, avg 0.15ms, max 1.40ms (capacity 16)
```

Roughly a hundredth of a frame budget. The browser is keeping these on the GPU
and `texImage2D` from a `VideoFrame` is a blit, not a copy. That is the good
outcome, and it is worth knowing rather than assuming, because the alternative
would have meant the whole design was wrong.

The measurement has an honest limitation: it is CPU time to submit, not GPU time
to complete, so a small number does not prove the GPU is idle. It does cleanly
separate the case we cared about, which was whether we had quietly introduced a
16MB memcpy per frame.

Every real bug in this story was found by instrumenting something rather than
thinking harder about it. The pool bug, the eviction policy, the extraction
race, the surface exhaustion and the decoder storm were all diagnosed from a
measurement, and in several cases the measurement contradicted a confident
explanation I had already written down. The instinct to reason first is the
expensive one here.

The tests deserve the same suspicion. Two of the regression tests I wrote for
these fixes passed against the broken code the first time, because the mock was
more forgiving than the real library: it produced its first sample instantly, so
it never reproduced a decoder that takes time to start. A test that cannot fail
is worse than no test, because it reads as coverage. Now every one of these is
checked against the pre-fix code before it is kept, and the decoder storm test
fails with `expected 20 to be less than or equal to 2`.

## The same engine, a different crash: opening the file

Playback was fixed. Then a 4K screen recording, 600 MB, three minutes, started
crashing the moment you opened it in the editor. Not a freeze this time. The
whole renderer process died with an out-of-memory abort. Smaller files opened
fine, which is the same tell as before: the cause scales with something about the
file, not with the code path.

The instinct in the room was to reach for a bigger hammer, a faster decoder, a
WebAssembly module, offload the work somewhere. All of that would have been
wasted, because nothing here was too slow. The problem was that everything
happened at the same instant.

When the editor opens a project, it kicks off the preview decoder, a second
decoder for the timeline filmstrip, a native `<video>` element buffering the same
file as a fallback, a scout decoder for cut-crossing, a Rust FFmpeg pass for the
thumbnail strip, and a cursor-analysis pass, and until we looked, all of them
fired in the same tick. On a small clip each is cheap. On a 4K file each is
heavy, and they landed on the same moment. Measured, the simultaneous peak was
about **1.85 GB**, most of it plain process memory that cannot be reclaimed until
it settles. On a machine with a constrained page file, that is the crash.

One allocation dominated, and it was the only one that scaled with *file size*
rather than resolution: the timeline filmstrip read the entire 600 MB file into an
ArrayBuffer, which the worker then copied into a Blob, about 1.2 GB transient,
600 MB resident for the whole session. That single line is why small files were
fine and the 4K one was not. The fix was not to make it faster. It was to stop
doing it: the filmstrip now range-streams the file through the same mechanism the
preview already used, and never holds it whole. That one change dropped roughly
1.2 GB from the peak. The owner's report afterwards was that the 4K file "loads
much faster now, almost as other files normally."

Then the rest of the pile-up: the load is staggered so the preview gets the main
thread first and the thumbnails, filmstrip, and cursor pass trickle in on idle, the pattern the waveform already used, just never applied to the decoders. And
the fallback `<video>` and `<audio>` elements dropped from `preload="auto"` to
`"metadata"`, because they were eagerly buffering a file that was not even on
screen. The spinner used to hide only when that `<video>` finished buffering, so
clearing it was load-bearing; now it hides the moment the preview paints a frame,
which is what it should have keyed on all along.

## The export that was sometimes fast

The same "measure, don't reason" rule turned up a bug that had nothing to do with
memory. Exports were slow, but only sometimes, and on the same machine. A job
that took forty seconds one run took two minutes the next.

Variance on identical hardware is a fingerprint. The export encoder is chosen by
probing the GPU encoders once and caching the winner for the whole app session.
The cache did not distinguish "this machine has no hardware encoder" from "the
hardware encoder was busy for a moment", and a consumer NVIDIA card only allows
a few simultaneous encoder sessions, so if a recording or preview was holding one
when the probe ran, it fell back to the software encoder and pinned it for the
rest of the session. Every export after that ran the CPU encoder at its slow
preset on 4K frames, five to ten times slower, silently, even after the GPU was
free again. The fix is small: cache a working hardware encoder, never cache the
software fallback, and cap the software preset so a GPU-less machine stays usable.

## What none of this was

It is worth saying plainly, because the reflex is strong. None of these fixes was
a WebAssembly problem, and we went looking for one honestly. There was no hot
JavaScript number-crunching loop anywhere in the load or playback path, decode
is the hardware decoder, compositing is one GPU draw call, the thumbnail and
waveform passes are already native FFmpeg. A crash from doing everything at once
is a *scheduling* problem, and scheduling is the one thing WebAssembly cannot
help with. The rest of the pass was the unglamorous kind: memoizing a timeline
computation that was rebuilt thousands of times per zoom, hoisting a handful of
per-frame allocations out of the 60 Hz draw loop, and coalescing scrub events to
one per frame instead of one per mouse movement. Each one small. None of them a
rewrite.

The thread through all of it is the same as the decoder story. The expensive
mistake is not the bug; it is the assumption that sends you looking in the wrong
place, that a frame is a picture, that a slow load needs a faster engine, that
variance is noise. Every real cause here was found by measuring, and every one of
them was cheaper to fix than the thing we would have built if we had trusted the
first instinct.

## Sources

- Paul Adenot, [WebCodecs performance](https://www.w3.org/2021/03/media-production-workshop/talks/paul-adenot-webcodecs-performance.html), W3C Media Production Workshop, 2021. Frame copy costs and the memory model.
- [Audio output latency](https://web.dev/articles/audio-output-latency), web.dev. `AudioContext.outputLatency` and syncing video to what the listener hears.
- [CapCut case study](https://web.dev/case-studies/capcut), web.dev. WebAssembly, SIMD and WebCodecs at scale.
- [Kapwing case study](https://web.dev/case-studies/kapwing), web.dev. WebCodecs decoding and local media caching.
- Alexsandro Souza, [Building a browser based video editor](https://www.linkedin.com/pulse/building-browser-based-video-editor-modern-web-alexsandro-souza-9vyde/). Frame cache sizing and worker architecture on the same stack we use.
- [Crashing the browser with 4K video](https://dev.to/will_indie/crashing-the-browser-with-4k-video-how-to-optimize-client-side-video-cropping-using-web-workers-pgg), dev.to. Frame lifecycle and backpressure under memory pressure.
- [MediaBunny](https://mediabunny.dev/), the demux and decode library this pipeline is built on.
- ITU-R BT.1359, relative timing of sound and vision, for the detectability thresholds behind our resync window.
