---
title: "Rebuilding Recast's editor preview on WebCodecs"
description: "We replaced the editor's video element with a decode pipeline we control: WebCodecs for video, a second decoder to hide cut latency, resolution-aware caching, range-based loading for large recordings, and a Web Audio engine that keeps sound aligned with cuts."
slug: rebuilding-editor-preview-on-webcodecs
date: 2026-06-27
author: Kanak
tags: [engineering, desktop, webcodecs, web-audio, video, tauri]
published: true
---

An HTML video element is very good at playing a video file. It is the wrong tool for playing an edited one, and it took us a visible freeze on every cut to admit it.

Recast's editor lets you cut sections out of a recording and play the result back before you export it. For a long time the preview that played that edited timeline was an ordinary HTML `<video>` element. It worked fine until you actually made a cut. Play across the gap and the picture froze for somewhere between half a second and a few seconds, then snapped back into motion. On a 4K screen recording the freeze was long enough that the feature felt broken.

This post is about fixing that, which turned into a rewrite of how the preview decodes video and plays audio. It covers the move from a `<video>` element to WebCodecs, a second decoder that hides the cost of crossing a cut, caching that adapts to resolution, a loading path for multi-gigabyte recordings, and a Web Audio engine that keeps audio lined up with the cuts. A few of these we got wrong the first time, and I will be specific about where.

## Why a video tag cannot do this

When you cut a section out of a timeline, playback has to skip it. With a `<video>` element the only tool for that is `video.currentTime = x`, which is a seek. A seek on a compressed stream is not free. H.264 stores most frames as differences from earlier frames, so to show the frame at a given point the decoder starts from the previous keyframe and decodes every frame up to the target. That work happens inside the element, on its own schedule, and you cannot see into it or steer it. The freeze at a cut was that seek running, with nothing to show until it finished.

There is a second problem that matters more once you chain edits together. A `<video>` element has one playhead and one clock. The edited timeline is a different clock. Mapping one onto the other through repeated seeks is a losing game, and it leaked into everything, audio included, which I will come back to.

## One timeline, two clocks

The change that made the rest possible was settling on a single way to talk about time.

There are two timelines in play. Original time is a position in the raw recording. Output time is a position in the edited result, where the cuts are already gone. A two second cut at the ten second mark means original time 12 maps to output time 10. We keep the playhead in original time, treat that as the single source of truth, and convert to output time with two small pure functions wherever a surface needs to present or seek in edited time. With no cuts the two are identical.

Every part of the preview now agrees on this. The scrubber, the playhead, the frame lookup, and the audio all derive from the same mapping instead of each holding its own idea of where playback is. That reads as obvious. It was not how the original code worked, and most of the sync bugs we had came from surfaces disagreeing about time.

## Decoding the video ourselves

To get rid of the seek-driven freeze, we took over decoding instead of leaving it to the element.

The recording is demuxed with mp4box in a Web Worker, which pulls out the encoded H.264 samples and the codec configuration. A WebCodecs `VideoDecoder` turns those samples into frames on demand. The frames are GPU backed, so the existing WebGL compositor uploads them the same way it uploaded a `<video>` element, just without the element in the middle. A clock running in output time drives the whole thing. It asks for the frame at the current time, the worker decodes toward it, and the compositor paints whatever frame is ready.

Crossing a cut is now a scheduling decision rather than a black box seek. We know the cut is coming because we own the clock, which means we can decode the frames after it ahead of time.

## The lesson that cost us the most: decoder surfaces

The first version of this was fast in testing and then dropped to about eight frames per second on real recordings. The cause is worth knowing if you ever build something similar.

A hardware video decoder hands back frames from a small pool of output surfaces. Each decoded frame you keep holds one of those surfaces. Hold too many and the pool runs dry, at which point the decoder accepts input and produces nothing, because it has nowhere to put the result. We had been caching frames generously to make scrubbing smooth, and the generosity was starving the decoder.

The fix was a small bounded cache: the frame on screen, a few frames ahead, and nothing else. This one constraint shaped almost every decision that followed, because every feature that wanted to hold more frames had to account for the surfaces it was using.

## Making cuts cheap: keyframes and a second decoder

With decoding under our control, the cut freeze split into two separate problems.

The first was the recording itself. We had never set a keyframe interval on the encoder, so it used the default, which lands a keyframe roughly every four seconds at sixty frames per second. Seeking to a cut meant decoding up to four seconds of frames to reach the cut point. We now record with a keyframe about every half second. That alone took the worst case from a few seconds down to a quarter second on average, and it sped up scrubbing and export seeks too. The cost is slightly larger files, which for mostly static screen content is a fair trade.

The second problem was the time still spent decoding after the jump. To hide it we added a second decoder, which we call the scout. As playback approaches a cut, the scout decodes the first frames after the cut in parallel with normal playback and parks them in a small protected cache. When the playhead reaches the cut, the frame it needs is already decoded, so there is nothing to wait for. The scout is strictly additive. If it fails for any reason it turns itself off, and playback falls back to the earlier behavior, which is a brief hold rather than a wrong picture. That property mattered, because the scout runs a second decoder against the same limited surface pool, and getting it wrong would have brought back the eight frames per second stall.

## Caching that knows the resolution

A fixed cache size is wrong across resolutions. Seven frames at 1080p is fine. Seven frames at 4K or 5K, which is what a Mac with a Retina display records at, holds four to seven times the surface memory and walks straight back into the starvation problem, made worse by the scout holding frames of its own.

So the cache size is no longer a constant. It is computed from the resolution against a fixed memory budget. At common resolutions it stays at the values we know are safe. At 4K and 5K it tightens, and the main cache, the scout's holdout, and how far ahead the worker decodes all shrink together, so the number of large surfaces in flight stays bounded. The math is a few lines and is unit tested, which is the only way to be sure about a calculation you cannot easily watch at runtime.

## Recordings that do not fit in memory

The original loader read the whole file into memory before decoding. For a normal clip that is fine. A long 4K recording is several gigabytes, and that approach runs the browser engine out of memory.

The fix is to read the file in pieces over HTTP range requests. Tauri's asset protocol already serves range requests, so this needed no new Rust code and no local server, which was the first design we considered and then dropped once we confirmed range requests worked. The worker fetches the index first. Our recordings put that index at the end of the file, because writing it at the front would stall the finalize step on large exports, so the loader reads from the tail to find it. From the index it builds a map of where every frame lives in the file, then fetches each group of frames on demand as playback reaches it, keeping recently used groups around within a byte budget so scrubbing stays cheap. Files under a size threshold still load whole, because the simpler path has no per-cut fetch latency and there is no reason to pay for streaming when the file fits.

## Audio, which we got wrong twice before getting it right

Video was only half of it. The recording's audio lives in separate WAV files for system sound and microphone, and they have to skip the same cuts the picture does.

The original approach slaved two `<audio>` elements to the playhead by seeking them, the same pattern we had just removed from video, and it failed the same way. Audio drifted around cuts. Short cuts were never corrected and the error accumulated. I tried to improve it and made it worse. An attempt to smooth drift by trimming playback rate also set a property called `preservesPitch`, and setting that property throws inside the app's web view, which aborted the play call before it ran and muted everything. A later attempt to snap harder at cuts stacked seeks during cold-start buffering and cut the audio out entirely. Three different failures, all rooted in the same idea of seeking a media element to chase a clock.

The right approach does not seek at all. We decode each WAV into an audio buffer once, then schedule each kept region of the timeline as its own source node on the audio hardware clock. A source node plays a slice of a buffer starting at an exact time, so the kept regions are scheduled back to back and the cuts are just the gaps between them. There is nothing to correct because nothing is chasing anything. Crossing a cut during normal playback does not even trigger a reschedule, because output time is continuous across a cut and the audio was already scheduled to be silent there. We rebuild the schedule only on a real scrub or when you edit a cut while playing. Like the scout, it falls back to the element based path if the audio context will not start or the files will not decode, so a failure means quieter handling, not silence.

## Where this stands

All of this sits behind an experimental toggle for now. It is solid on the machine we develop on, and the parts that can be tested without a running browser are unit tested: the time mapping, the frame index math, the cache sizing, the audio scheduling. The parts that cannot, mainly the behavior of WebCodecs and Web Audio inside the app's web view at 4K on a Mac and under WebKitGTK on Linux, are the work between here and turning the toggle on by default.

The thread running through the whole rewrite is that we stopped asking the browser's built-in media elements to play an edited timeline and started treating decode, scheduling, and playback as things we own. The elements are good at playing a file. They are not built to play a timeline with holes in it, on a clock you control. Once we accepted that, most of the work was bounding resources carefully and being honest about the cases we could not yet test.
