---
kind: post
title: "Teaching Recast to read its own recordings"
description: "We want an agent to understand what happened in a screen recording, frame by frame, even when there is no narration to lean on. This is the research behind that feature: why we anchor on pixels instead of audio, why we picked a text-first pipeline over feeding screenshots to a vision model, and the two things I had backwards before I looked them up."
slug: teaching-recast-to-read-its-recordings
date: 2026-07-13
author: Kanak
tags: [engineering, desktop, ocr, agents, research, video, tauri]
published: true
---

Recast is heading toward automation. The plan is that you can drive it from a command line, and eventually let an agent drive it for you: record this, trim that, export with these settings. For any of that to be more than a remote control, the agent has to understand what is on the screen. Not the file, the screen. What window is open, what text is showing, what the user just clicked.

The obvious way to get that understanding is to listen to the recording. Most of our caption work already does exactly this, and it works well. Transcribe the audio, get word-level timestamps, and now you have a timeline of what was said and when. But a screen recording is not a talk. Half the recordings people make have no narration at all. Someone is clicking through a settings page in silence, or the mic was off, or they plan to record voiceover later. If the only thing anchoring your understanding of the video is the audio, then a silent recording is a video you understand nothing about.

So the anchor has to be something that is always there. That is the pixels, and the cursor.

## Why the pixels have to be the spine

There are three signals in a Recast recording that exist whether or not anyone spoke. The frames themselves, which are always present. The cursor path and click events, which we already capture at 125 samples a second and store next to the video. And the captions, which only exist when there was audio to transcribe.

Ranked by how much you can rely on them, the frames come first. A recording always has frames, even an imported one that never touched our capture pipeline. So the frames become the backbone of the timeline, and everything else layers on top when it happens to be available. Captions enrich the timeline when there is sound. The cursor enriches it when we recorded it. But the structure of the video, the sense of "this is what the screen looked like from second twelve to second twenty," comes from reading the frames.

That decision sounds small. It quietly rules out the entire approach most caption systems take, which is to build the timeline from the transcript and hang everything else off it. We are building the timeline from the picture and hanging the transcript off that.

## Two ways to show a model a screen

Once you commit to reading the picture, the next question is what "reading" means, and here I went and read what everyone else is doing, because this is a crowded field right now and I did not want to reinvent a worse version of a solved problem.

There are two camps, and they are further apart than I expected.

The first camp takes a screenshot and hands it, as an image, straight to a vision model. This is what Claude Computer Use does, and what OpenAI's computer-use model does, and what ByteDance's UI-TARS does. No text extraction, no structure, just pixels into a model that was trained to look at screens and say where to click. The interesting part is that the frontier is moving further in this direction, not less. The recommendations now are to give the model a higher resolution screenshot and let it ask for a zoomed-in crop when the text is too small, rather than to pre-process the image into something smaller. The bet is that the vision model is good enough that parsing the screen yourself only throws away information.

The second camp turns the screenshot into a structured list first. Microsoft's OmniParser is the clearest example. It runs OCR and an icon detector over the frame and produces a list of elements: this box at these coordinates contains this text and is clickable, that box is an icon that means "settings." A model, any model, reads that list and decides what to do. The list is the interface.

The catch with the first camp is the one that matters for us. Feeding screenshots to a vision model means you need a vision model, and a good one, which today means a two-billion to seventy-billion parameter model that wants a GPU. Every strong end-to-end screen model is that size. Even Apple's on-device attempt is three billion parameters and does not run anywhere but their own hardware. Recast is a desktop app that has to work on a five-year-old Intel MacBook with no discrete graphics. We cannot ship a model like that and have it finish before the user gives up.

So we are in the second camp by necessity, not preference. We read the screen into a structured list on the device, cheaply, and let the real reasoning happen in the cloud model the agent already talks to. OCR is not the whole answer anymore, the way it might have been five years ago. It is one cheap tier that feeds a much smarter reader.

## The efficiency story I had backwards

I went into this assuming the structured list was also the cheaper option, token for token. A screenshot must cost a fortune to send to a model, and a tidy list of text must be a fraction of that. That assumption was wrong, and it is worth saying so plainly because it changed how I describe the feature.

A screenshot sent to Claude costs somewhere between a thousand and eighteen hundred tokens. A structured list of every element on a busy screen, with coordinates and text for each one, can easily cost more than that. On a dense settings page it is not close. The list is not the cheap option.

What the list actually buys you is different, and better than a token saving. It works with a text-only model that has no vision at all, which matters when the agent driving Recast is a tool-calling model rather than a multimodal one. It is auditable, because you can read exactly what the machine thought was on the screen and when, instead of trusting a black box that looked at a picture. And it costs no GPU per frame, which for a video with hundreds of sampled frames is the difference between a feature that runs on the device and one that does not. OmniParser is fast, about six tenths of a second a frame, but that number is on a data-center GPU. Multiply it by every frame in a video, on a laptop, and the screenshot-to-vision-model approach stops being an option at all.

So the pitch for this feature was never "it is cheaper." It is "it runs on your machine, it works without narration, and you can read what it decided." I would rather say that accurately than oversell a token count that does not hold up.

## Not storing the picture, but keeping a way back to it

There is a version of this feature for text-only models, which is the structured list, and there is a version for multimodal models, which can also use the actual image as a reference. We want both, and the tension between them is storage. We are not building a system that hoards a full-resolution frame for every second of every recording. That is a lot of disk for something you might never look at.

The way out came from reading how the vision models actually treat images. Claude sees an image as a grid of twenty-eight pixel patches, and anything larger than about fifteen hundred pixels on its long edge gets scaled down before the model ever sees it. That is the whole trick. If the model is going to shrink a big image down to that size anyway, then storing a big image is pure waste. A frame saved as a WebP at around twelve hundred pixels wide, at reasonable quality, is a few tens of kilobytes and is, as far as the model is concerned, indistinguishable from the original. It is not a thumbnail in the lossy, throw-away sense. It is the largest thing the model would ever actually use, and no larger.

So the structured list is the thing we always keep. The image is optional, small, and only written when someone turns on the multimodal path. You never store the heavy frame, and you never lose the ability to show the model a picture, because the small one is all it wanted.

## The change detector I got wrong first

The naive way to sample a video is to look at every frame. A screen recording at sixty frames a second, run through OCR, is a staggering amount of duplicated work, because the screen does not change sixty times a second. It changes a few times, when you click something or a dialog opens, and sits still the rest of the time. So the real job is to notice when the screen changed and only read those moments.

My first sketch for "did the screen change" was to turn each frame gray and measure how different it was from the last one. Simple, fast, and, it turns out, wrong in a way I would not have caught without looking at how the established tools do it. PySceneDetect, which is the reference for this, does not just look at brightness. It compares hue, saturation, and brightness together. My gray-only version was one third of the real thing, and the missing two thirds are exactly the changes a screen recording is full of. A theme switching from light to dark can keep the overall brightness almost the same while every color inverts. A dialog in a similar shade opening over the page. Syntax highlighting recoloring a block of code. All of those are obvious to a person and invisible to a brightness-only comparison.

The second thing the reference tools do is smarter thresholding. Instead of a fixed "changed by more than this much" line, they divide each frame's difference by the recent average, and trigger when that ratio spikes. This is what keeps a slow scroll from registering as a thousand tiny changes, while a genuine cut still stands out. A fixed threshold either fires constantly on smooth motion or misses gradual changes, and there is no single number that avoids both.

The last piece is cheap and I nearly skipped it. Before doing any of the color math, hash each frame with a gradient hash, the kind that is robust to small brightness shifts, and if it is nearly identical to the last frame you kept, drop it outright. It kills the easy duplicates for almost no cost before the more careful comparison runs.

None of this is novel. It is what the video tools have done for years. The lesson was that my clever-enough first idea was measurably worse than the boring correct one, and the only reason I know is that I read the source instead of trusting the sketch.

## Which OCR engine, and the trap I avoided

We already had an OCR engine wired up, a pure-Rust library called ocrs, chosen months ago for one specific reason: it runs on that old Intel Mac. The more accurate options all lean on a runtime that has no build for Intel Macs, and we had already been burned by exactly that when we ripped out our old speech engine for the same reason. So ocrs was the safe floor.

Two things came out of researching whether we were using it right.

The first is that we were, and I had half-convinced myself we were not. ocrs has a one-line call that gives you all the text as a single string, and a longer four-step version that gives you the text plus the box around every line and word. I had wondered if reaching for the longer version was a misuse. It is not. The longer version is the documented way to get the coordinates, and coordinates are the entire point of a structured list. The short call is only for when you want a flat string and nothing else.

The second is that ocrs is the safe choice, not the best one. It describes itself as an early preview, it only handles the Latin alphabet, and its text model is trained on data that carries a share-alike license, which is a small but real obligation to track. For the crisp, high-resolution text a screen recording produces, the operating system's own OCR is meaningfully better, ships no model to download, and carries no license baggage. Apple's Vision framework, which works on Intel Macs because it is an OS feature rather than a bundled model, and Windows' built-in OCR, are both more accurate on exactly the kind of clean UI text we are reading.

There was one tempting wrong turn here. The most accurate open option is a PaddleOCR model, and there is a Rust crate that runs it. But it only runs on the same runtime that has no Intel Mac build, the one we already fled once. Adopting it would reopen the exact wound we spent effort closing. So the answer is a small interface with ocrs behind it today, because it already works everywhere, and the native OS engines added behind the same interface next, for the two platforms where they are clearly better. Not the accurate-but-fragile model that would cost us the machine we most need to support.

## What we are building first

The first slice is deliberately narrow, and firmly in the structured-list camp. Take a recording, sample it down to the frames where the screen actually changed, read the text and its coordinates out of each of those frames, and collapse runs of near-identical frames into spans. The output is a timeline: from this second to that second, the screen showed this text at these positions. Coordinates are stored as fractions of the frame rather than pixels, so they mean the same thing regardless of the recording's resolution, the way the research systems do it. The shape borrows directly from OmniParser's element list, because it is a proven format that a plain text model can act on.

No cursor in this first slice, no stored images, no OS-native OCR yet. Those all layer on cleanly afterward, precisely because the spine does not depend on any of them. The cursor becomes an enrichment pass that labels which piece of text a click landed on, and feeds click moments back in as frames we always read even when the picture barely changed. The images become an opt-in for the multimodal path. The native OCR engines slot in behind the same interface. But the thing we prove first is the boring core: video in, a readable timeline out, running on the device, with no audio required.

The thread through all of it is that the interesting decisions were the ones where the honest answer was less exciting than the first idea. The screenshot-into-a-vision-model approach is genuinely where the frontier is, and it is the wrong fit for a laptop app, so we took the older text-first road on purpose. The structured list is not cheaper, it is just usable by more kinds of model, so that is how we describe it. My gray-frame change detector was clever and worse than the color-aware one that has existed for a decade. And the most accurate OCR would have cost us the exact machine we started this whole engine choice to protect. A few days of reading before writing any of the real code found all of that, which is a much better trade than finding it in the field.
