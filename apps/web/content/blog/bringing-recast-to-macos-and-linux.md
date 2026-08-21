---
kind: post
title: "Bringing Recast to macOS and Linux"
description: "Recast started as a Windows app. Here is what it took to run it natively on macOS and Linux: the two decisions that kept it from becoming a mess, the freeze that only happened on a Mac, and exactly where each platform stands today."
slug: bringing-recast-to-macos-and-linux
date: 2026-06-27
author: Kanak
tags: [engineering, cross-platform, desktop, tauri, rust]
published: true
---

Most of the work in making one app run well on three operating systems is not writing platform code. It is finding out which of your assumptions were really just Windows in disguise.

Recast started as a Windows app. That was a default more than a decision. You build on the machine in front of you, and the shortest path to record a screen, polish it, and share it ran through the Windows capture and audio APIs. But a recorder that only runs on one operating system is a demo, not a product. So we set out to make Recast run natively on macOS and Linux too, with the same record, polish, and share loop on whatever you happen to be using.

Here is what that took, and, because we would rather be straight with you than sell you a roadmap, where each platform actually stands.

## Two decisions that kept it from becoming a swamp

Cross-platform native code has a reputation for being miserable to write. Ours was not, and the reason comes down to two decisions we made early without fully understanding how much they would pay off.

The first is a platform module per capability. Screen capture, audio, camera, and cursor tracking are each a folder of small files, one per operating system, behind a single interface and a compile-time switch. Adding macOS screen capture did not mean touching the Windows path. It meant writing one new file that satisfied the same contract. Every gap was additive and isolated, so there was no large refactor and no day where everything was broken at once.

The second is FFmpeg as the shared layer. Underneath the platform-specific capture front ends, every operating system hands raw frames and audio samples to the same FFmpeg based encode and format pipeline. The Windows capture API, AVFoundation on macOS, and PipeWire and X11 on Linux are four different ways to get pixels, and they all feed one encoder. The hard, platform-specific part shrinks to one job: get me frames. Everything after that is shared.

With that shape, the macOS and Linux build out went faster than we expected. Screen capture, system audio, microphone, camera, cursor tracking, and device pickers are implemented on all three, and they compile on every push in CI.

## The bug that only existed on a Mac

The most useful bug of the whole effort never reproduced on Windows.

A macOS tester reported that the app froze the moment a recording finished. The window stopped responding and clicks went nowhere. On Windows the same build was fine. We had two obvious suspects, a UI library quirk and an FFmpeg subprocess, and both were wrong.

The real cause was one missing keyword. The command that stops a recording was a synchronous function, and inside it the app did real work: flushing the encoder, finalizing the file, and sometimes a re-encode that takes a few seconds. On Windows that is invisible, because Windows runs the web interface in a separate process that keeps painting no matter what the backend is doing. macOS and Linux run the interface on the same main thread the command runs on. Block that thread and the whole window locks up until the work finishes.

The fix was to move the heavy work onto a background thread so the interface thread stays free. The larger lesson was that this was not one bug. We went back through the rest of the recording code looking for the same pattern and found more of it: listing audio devices, listing recordings, revealing a file in its folder. Each was a freeze waiting to happen that Windows had been quietly hiding for us. We found a related one too. On a long recording, FFmpeg's own progress output could fill an operating-system pipe buffer that nothing was draining, which stalled the encode partway through capture. None of these were macOS bugs. They were our bugs, and macOS and Linux were just honest enough to show them.

That is the pattern of going cross-platform. Other operating systems do not only run your code. They check it.

## Where each platform stands

We would rather tell you the truth than show you a wall of green checkmarks.

Windows is done. It is the reference platform, it is in people's hands, and it is verified.

macOS has all of its capture code written and a tester pass underway. The honest gaps are hardware verification of the permissions flow and of Retina and multi-monitor behavior, the system-audio default, and signing. System audio is the awkward one. macOS has no built-in loopback the way Windows does, so capturing the sound coming out of the speakers needs a different approach than the other platforms, and we are finishing that path. Signing and notarization are the difference between an app you have to right-click past Gatekeeper and one that just opens, and that is a grind of its own.

Linux is code complete and waiting on its first real hardware pass. Wayland, through the desktop portal and PipeWire, and X11 are both written and have already been through a round of audit fixes, but nothing earns a checkmark from us until it has actually run on a real GNOME, KDE, and X11 session.

The short version: Windows ships today, macOS is a focused round of verification and signing away from a public preview, and Linux is one hardware bring-up behind that.

## Why we are showing the middle

Most "now on macOS and Linux" posts go up the day everything is finished. We are writing this one earlier on purpose. Recast is founder built, and we would rather show you the actual engineering, the abstractions, the war stories, and the honest gaps, than a press release. The code is written. The remaining distance is a person sitting in front of a Mac and a Linux machine, doing the unglamorous work of proving each path on real hardware.

If you are on macOS or Linux and want to help us get there sooner, that kind of early feedback is exactly what moves a platform from written to verified. We will have preview builds to share soon.
