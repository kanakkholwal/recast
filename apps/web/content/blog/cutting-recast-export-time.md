---
kind: post
title: "Cutting Recast's export from nearly six minutes to under two"
description: "A 46-second recording took five minutes and forty-two seconds to export on a machine with a hardware encoder. This is how we found the real cause, threw out a confident wrong guess about threading on the way, and got it down to a minute and a half."
slug: cutting-recast-export-time
date: 2026-07-02
author: Kanak
tags: [engineering, desktop, ffmpeg, performance, video, tauri]
published: true
---

A 46-second screen recording took five minutes and forty-two seconds to export. The machine doing the work has an NVIDIA hardware H.264 encoder, the kind that can turn out a clip like that in ten or fifteen seconds. So the encoder was not the problem, even though for a while it looked like the only suspect. Getting from that number down to a minute and a half turned out to be a story about not trusting the obvious explanation.

## Measure before touching anything

The first thing we did was add per-stage timing to the export command, because "it's slow" is not something you can act on. The breakdown for that clip: preparation 1.3 seconds, cursor overlay pre-render 48.6 seconds, main encode 291.7 seconds, total 341.7. We also logged which encoder actually ran, and it was `h264_nvenc`. Hardware.

That last fact is the whole story in miniature. The encode took 291 seconds for 46 seconds of video, roughly six times slower than real time, on a GPU encoder that should run several times faster than real time. An encoder that slow is almost never the bottleneck. It is idle, waiting on something upstream to hand it frames. The thing upstream is the FFmpeg filter graph, running on the CPU, and it was starving the GPU.

## The obvious fix that did nothing

If the filter graph is the CPU-bound part, you parallelize it. So we set `-filter_complex_threads` and `-filter_threads` to the core count and added `-hwaccel auto` to move decoding off the CPU and onto the GPU. Re-export. 296 seconds. No change at all.

The negative result was worth more than a small win would have been. `-filter_complex_threads` already defaults to the number of cores, so setting it explicitly did nothing, and decoding was never the bottleneck, so offloading it did nothing either. We reverted the `-hwaccel` change rather than keep a decode path that added a platform-specific edge case for zero benefit. Had we shipped on intuition, we would have carried complexity for a measured zero percent.

## Bisecting the graph

Since threading was a no-op, the cost had to be genuine per-frame work that was already using every core. Rather than guess a second time, we profiled the graph directly. We generated a synthetic 120fps clip, ran the real filter chain against it with the output thrown away, and timed each stage in isolation.

Per frame: decoding was under a millisecond. Compositing the video onto a static background was about six milliseconds. The same background with its per-frame blur was twenty-five. The blur was the cost, nineteen and a half milliseconds a frame, on an image that never changes.

The background is a still wallpaper, handed to FFmpeg as a looped image. The graph was running scale, crop, and `boxblur` on it for every frame, roughly 5,600 times across this clip, computing the identical result each time. FFmpeg has no way to know the input is static, so it did the work over and over.

One more thing fell out of the same benchmark, and it is worth telling because it saved us from a plausible second mistake. Recast's zoom and scene animations compile into large piecewise-linear expressions, hundreds of terms each, evaluated once per frame. I assumed those were expensive and had already sketched a plan to shrink them. The benchmark said the reverse: the graph with the big expression ran faster than the same graph with a trivial one. The reason is that the big expression sits at its resting value most of the time, which the scaler treats as identity and skips, while a trivial `sin()` changed the scale on every frame and forced a real resample. The expression size was never the cost. The resampling was, and only when the scale actually moved. That whole line of work was a dead end, and the measurement told us before we spent a day on it.

## Baking the background once

The fix writes the fully processed background, scaled and cropped and blurred, to a single PNG before the encode starts, then loops that image with the blur turned off. It is the same render-once pattern we already use for the drop shadow and the gradient backgrounds. The result is pixel identical, because it is the exact same filter chain run one time instead of 5,600 times.

Re-export: the encode dropped from 291.7 seconds to 130.4, and the total from 5m42 to 2m48. That is a 56 percent cut on the encode, better than the benchmark predicted, because baking the background also removed the per-frame scale and crop of the wallpaper, not only the blur.

## The frame rate nobody asked for

The recording is 120fps, and export defaulted to matching the source, so it was encoding 120 frames a second. For a screen recording that is invisible. Nobody watching a product walkthrough can tell 60 from 120, and the extra frames double the work through the entire graph and the encoder.

So we changed the export default to 60fps for any recording above 60, while leaving Original, 30, and 24 there for anyone who wants them. It is seeded once when the recording loads, so it never overrides a rate you pick yourself.

This halves both of the big costs at the same time, the encode and the cursor pre-render, because both scale with frame count. The total went from 2m48 to 1m37. Against the original 5m42, that is about three and a half times faster.

## Making the wait honest

Two smaller changes, both about the export not lying to you.

The progress bar measured percent against the raw trimmed length of the clip. But the file we actually write is the edited length, with cuts removed and speed applied, which is what we tell FFmpeg to produce with `-t`. On a clip with a cut those two lengths differ, so the bar would stall short of 100 percent, and on a slowed clip it would hit 100 early and sit there. We now measure progress against the real output length, the same value we pass to `-t`.

And the export does real work before the encoder ever starts. It renders a full cursor and annotation layer for the whole timeline, which at 60fps is around eighteen seconds. During that stretch the bar had nothing to report and sat blank, which reads as a hang. The dialog already had a step checklist for the earlier browser-side preparation, so we extended it into the backend: the export now names the step it is on, and the checklist shows "Rendering cursor and annotations" ticking over before it moves on to "Encode frames". Eighteen seconds of visible, named work is a completely different experience from eighteen seconds of a frozen bar, even though the clock is the same.

## Where this stands

5m42 to 1m37 on the test clip, roughly three and a half times faster, with byte-identical output at the same frame rate and honest output at 60.

There is one more lever we chose not to pull. The cursor layer is still rendered to an intermediate file and read back in, and it could instead be streamed straight into the encoder as it renders, overlapping the two passes. That would save maybe fifteen seconds. It also means piping raw frames into FFmpeg's stdin from a background thread, which on Windows is precisely the kind of thing that works on the dev machine and deadlocks in the field, and we could not confirm it without shipping it. The payoff was small enough, and the checklist had already fixed the part that actually felt bad, that we banked the wins and left it.

The thread running through all of it is that the slow part was never where it looked. The encoder looked guilty and was innocent. Threading looked like the fix and did nothing. The real culprit was a static image being blurred sixty times a second, which no amount of reading the code would have surfaced. A few hours of measurement found it, and a one-time PNG fixed it.
