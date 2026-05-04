<script lang="ts">
  import type { Easing } from "$lib/easing/cubic-bezier";
  import type { EditorStore, ZoomRegion } from "$lib/stores/editor-store.svelte";
  import { CircleQuestionMark, Gauge, Scissors, Search, Wand2, X, ZoomIn, ZoomOut } from "@lucide/svelte";
  import { Badge } from "@recast/ui/badge";
  import { Button } from "@recast/ui/button";
  import * as DropdownMenu from "@recast/ui/dropdown-menu";
  import { Kbd } from "@recast/ui/kbd";
  import { cn } from "@recast/ui/utils";
  import {
    HoverCard,
    HoverCardContent,
    HoverCardTrigger,
  } from "@recast/ui/hover-card";
  import { onMount } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { fade, fly } from "svelte/transition";
  import ZoomSuggestionsPopover from "./ZoomSuggestionsPopover.svelte";
  interface Props {
    store: EditorStore;
    videoEl?: HTMLVideoElement | null;
  }

  let { store, videoEl = null }: Props = $props();

  let timelineEl: HTMLDivElement | undefined = $state();
  let isDraggingPlayhead = $state(false);
  let showSuggestions = $state(false);
  let timelineWidth = $state(900);
  let activeTrimHandle = $state<"in" | "out" | null>(null);

  // Live drag context for the in/out trim handles. `originalAt` is the value
  // the handle had at pointer-down — used to display a delta in the tooltip
  // so users see exactly how many frames they've shaved off.
  let trimDragContext = $state<{
    which: "in" | "out";
    originalAt: number;
  } | null>(null);

  const SPEEDS = [0.25, 0.5, 1.0, 1.5, 2.0];
  let playbackSpeed = $state(1.0);

  // JKL transport: cycles 1×→2×→4× on each consecutive press, like Avid /
  // Premiere. K parks playback. We don't drive reverse playback through
  // <video>'s playbackRate (browsers don't support negative rates reliably);
  // J instead schedules a rAF loop that decrements currentTime.
  let shuttleDirection = $state<-1 | 0 | 1>(0);
  let shuttleSpeedIndex = $state(0);
  const SHUTTLE_SPEEDS = [1, 2, 4];
  let reverseFrame = 0;

  $effect(() => {
    if (videoEl) {
      videoEl.playbackRate =
        shuttleDirection === 1
          ? SHUTTLE_SPEEDS[shuttleSpeedIndex] * playbackSpeed
          : playbackSpeed;
    }
  });

  // Reverse-play loop. Held active only while shuttleDirection === -1.
  function pumpReverse() {
    if (shuttleDirection !== -1 || !videoEl) {
      reverseFrame = 0;
      return;
    }
    const fps = effectiveFps();
    const step = (SHUTTLE_SPEEDS[shuttleSpeedIndex] / fps) * playbackSpeed;
    const next = Math.max(store.inPoint, store.currentTime - step);
    store.currentTime = next;
    videoEl.currentTime = next;
    if (next <= store.inPoint) {
      shuttleDirection = 0;
      shuttleSpeedIndex = 0;
      reverseFrame = 0;
      return;
    }
    reverseFrame = requestAnimationFrame(pumpReverse);
  }

  $effect(() => {
    if (shuttleDirection === -1 && reverseFrame === 0) {
      reverseFrame = requestAnimationFrame(pumpReverse);
    } else if (shuttleDirection !== -1 && reverseFrame !== 0) {
      cancelAnimationFrame(reverseFrame);
      reverseFrame = 0;
    }
  });

  // Quantization helpers. All trim and playhead writes round to the nearest
  // frame boundary so preview and export agree on which exact frame is the
  // first/last kept frame. Sub-frame trim values are the source of off-by-one
  // mismatches between scrub preview and the rendered MP4.
  function effectiveFps(): number {
    const f = store.metadata?.fps ?? 0;
    return f > 0 ? f : 60;
  }
  function quantizeToFrame(time: number): number {
    const fps = effectiveFps();
    return Math.round(time * fps) / fps;
  }
  function frameStep(): number {
    return 1 / effectiveFps();
  }
  // SMPTE-style HH:MM:SS:FF (or MM:SS:FF for clips < 1 hour). Frame component
  // is zero-padded so the readout has constant width.
  function formatTimecode(time: number): string {
    const fps = effectiveFps();
    const t = Math.max(0, time);
    const totalFrames = Math.round(t * fps);
    const frames = totalFrames % Math.round(fps);
    const totalSecs = Math.floor(totalFrames / Math.round(fps));
    const secs = totalSecs % 60;
    const mins = Math.floor(totalSecs / 60) % 60;
    const hours = Math.floor(totalSecs / 3600);
    const ff = String(frames).padStart(2, "0");
    const ss = String(secs).padStart(2, "0");
    const mm = String(mins).padStart(2, "0");
    return hours > 0
      ? `${String(hours).padStart(2, "0")}:${mm}:${ss}:${ff}`
      : `${mm}:${ss}:${ff}`;
  }
  // Floor on the clip length: at least 2 frames so the trimmed range is
  // never sub-frame. Scales naturally with fps (60fps → ~33ms; 30fps → ~66ms).
  function minClipDuration(): number {
    return 2 * frameStep();
  }

  function zoomTimeline(dir: number) {
    store.timelineZoom = Math.max(0.5, Math.min(5, store.timelineZoom + dir * 0.25));
  }

  function toggleSuggestions() {
    showSuggestions = !showSuggestions;
  }

  function clientXToTime(clientX: number): number {
    if (!timelineEl || duration <= 0) return 0;
    const rect = timelineEl.getBoundingClientRect();
    const scrollLeft = timelineEl.scrollLeft;
    const x = clientX - rect.left + scrollLeft;
    return Math.max(0, Math.min(duration, x / pixelsPerSecond));
  }

  function startTrimDrag(event: PointerEvent, which: "in" | "out") {
    if (duration <= 0) return;
    event.preventDefault();
    event.stopPropagation();
    // Single undo entry per drag, regardless of how many pointermove events
    // fire while the user holds the handle.
    store.pushUndoState();
    activeTrimHandle = which;
    trimDragContext = {
      which,
      originalAt: which === "in" ? store.inPoint : store.outPoint,
    };
    document.body.style.cursor = "ew-resize";
    (event.currentTarget as Element).setPointerCapture(event.pointerId);
    updateTrimFromPointer(event.clientX, which, true);
    const onMove = (e: PointerEvent) => {
      updateTrimFromPointer(e.clientX, which, true);
    };
    const onUp = (e: PointerEvent) => {
      activeTrimHandle = null;
      trimDragContext = null;
      document.body.style.cursor = "";
      try {
        (event.currentTarget as Element).releasePointerCapture(e.pointerId);
      } catch {
        // already released on some browsers
      }
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
  }

  function updateTrimFromPointer(
    clientX: number,
    which: "in" | "out",
    scrub = false,
  ) {
    const raw = clientXToTime(clientX);
    const t = quantizeToFrame(raw);
    const min = minClipDuration();
    if (which === "in") {
      const next = Math.max(0, Math.min(t, store.outPoint - min));
      store.trimStart = next;
      // Scrub-while-trim: park playback at the in point so the preview
      // shows the first kept frame as the user drags.
      if (scrub) {
        store.currentTime = next;
        if (videoEl) videoEl.currentTime = next;
      }
    } else {
      const next = Math.min(duration, Math.max(t, store.inPoint + min));
      store.trimEnd = next;
      if (scrub) {
        // Show one frame before the cut (the last kept frame) — that's the
        // frame the user is actually deciding to keep or discard.
        const previewAt = Math.max(store.inPoint, next - frameStep());
        store.currentTime = previewAt;
        if (videoEl) videoEl.currentTime = previewAt;
      }
    }
  }

  function handleTrimHandleKey(event: KeyboardEvent, which: "in" | "out") {
    if (duration <= 0) return;
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    event.stopPropagation();
    nudgeTrim(which, event.key === "ArrowLeft" ? -1 : 1, event.shiftKey);
  }

  // Shared trim-nudge for both the handle's own arrow keys and the global
  // Alt+[ / Alt+] shortcuts. Direction is ±1; shift switches the unit from
  // one frame to one second.
  function nudgeTrim(which: "in" | "out", direction: 1 | -1, second = false) {
    if (duration <= 0) return;
    store.pushUndoStateCoalesced(`trim-${which}`, 500);
    const delta = direction * (second ? 1 : frameStep());
    const min = minClipDuration();
    if (which === "in") {
      const next = quantizeToFrame(
        Math.max(0, Math.min(store.outPoint - min, store.inPoint + delta)),
      );
      store.trimStart = next;
    } else {
      const next = quantizeToFrame(
        Math.max(store.inPoint + min, Math.min(duration, store.outPoint + delta)),
      );
      store.trimEnd = next;
    }
  }

  const duration = $derived(store.metadata?.duration ?? 0);
  const pixelsPerSecond = $derived(
    duration > 0 ? (timelineWidth * store.timelineZoom) / duration : 100,
  );
  const totalWidth = $derived(
    Math.max(duration * pixelsPerSecond, timelineWidth),
  );
  const playheadLeft = $derived(store.currentTime * pixelsPerSecond);
  const clipLeft = $derived(store.inPoint * pixelsPerSecond);
  const clipRight = $derived(store.outPoint * pixelsPerSecond);
  const clipWidth = $derived(Math.max(clipRight - clipLeft, 0));
  const thumbnailWidth = $derived(
    store.thumbnailStrip.length > 0
      ? Math.max(88, clipWidth / store.thumbnailStrip.length)
      : 112,
  );
  const hasTrim = $derived(
    duration > 0 && (store.inPoint > 0 || store.outPoint < duration),
  );
  const frameCount = $derived(
    Math.max(
      0,
      Math.round((store.metadata?.duration ?? 0) * (store.metadata?.fps ?? 0)),
    ),
  );
  const aspectRatioLabel = $derived.by(() => {
    const width = store.metadata?.width ?? 0;
    const height = store.metadata?.height ?? 0;
    if (!width || !height) return "Source";
    const divisor = greatestCommonDivisor(width, height);
    return `${Math.round(width / divisor)}:${Math.round(height / divisor)}`;
  });

  const timeMarkers = $derived.by(() => {
    if (duration <= 0) return [];
    const markers: { time: number; label: string; emphasis: boolean }[] = [];

    let interval = 1;
    if (pixelsPerSecond < 26) interval = 10;
    else if (pixelsPerSecond < 52) interval = 5;
    else if (pixelsPerSecond < 120) interval = 2;
    else if (pixelsPerSecond > 260) interval = 0.5;

    for (let t = 0; t <= duration + interval * 0.5; t += interval) {
      const mins = Math.floor(t / 60);
      const secs = Math.floor(t % 60);
      markers.push({
        time: t,
        label: `${mins}:${secs.toString().padStart(2, "0")}`,
        emphasis: Math.round(t) % (interval >= 2 ? interval * 2 : 2) === 0,
      });
    }

    return markers;
  });

  const minorTicks = $derived.by(() => {
    if (duration <= 0) return [];
    const ticks: number[] = [];
    const interval =
      pixelsPerSecond > 180 ? 0.25 : pixelsPerSecond > 80 ? 0.5 : 1;
    for (let t = 0; t <= duration + interval * 0.5; t += interval) {
      ticks.push(t);
    }
    return ticks;
  });

  function greatestCommonDivisor(a: number, b: number): number {
    let left = Math.abs(a);
    let right = Math.abs(b);
    while (right !== 0) {
      const next = left % right;
      left = right;
      right = next;
    }
    return left || 1;
  }

  function formatTime(seconds: number) {
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    const centiseconds = Math.floor((seconds % 1) * 100);
    return `${mins}:${secs.toString().padStart(2, "0")}.${centiseconds
      .toString()
      .padStart(2, "0")}`;
  }

  function seekToPosition(clientX: number) {
    if (!timelineEl || duration <= 0) return;
    const rect = timelineEl.getBoundingClientRect();
    const scrollLeft = timelineEl.scrollLeft;
    const x = clientX - rect.left + scrollLeft;
    const time = Math.max(0, Math.min(duration, x / pixelsPerSecond));
    store.currentTime = time;
    if (videoEl) videoEl.currentTime = time;
  }

  function handleTimelinePointerDown(event: PointerEvent) {
    isDraggingPlayhead = true;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    seekToPosition(event.clientX);
  }

  function handleTimelinePointerMove(event: PointerEvent) {
    if (!isDraggingPlayhead) return;
    seekToPosition(event.clientX);
  }

  function handleTimelinePointerUp() {
    isDraggingPlayhead = false;
  }

  function handleTimelineKeydown(event: KeyboardEvent) {
    if (duration <= 0) return;

    const step = event.shiftKey ? 1 : frameStep();

    if (event.key === "ArrowLeft" && !event.altKey) {
      event.preventDefault();
      const next = quantizeToFrame(Math.max(0, store.currentTime - step));
      store.currentTime = next;
      if (videoEl) videoEl.currentTime = next;
    }

    if (event.key === "ArrowRight" && !event.altKey) {
      event.preventDefault();
      const next = quantizeToFrame(
        Math.min(duration, store.currentTime + step),
      );
      store.currentTime = next;
      if (videoEl) videoEl.currentTime = next;
    }

    // Premiere-style in/out point shortcuts.
    if (event.key === "i" || event.key === "I") {
      event.preventDefault();
      if (event.shiftKey) {
        store.pushUndoState();
        store.trimStart = 0;
      } else {
        setTrimPoint("in");
      }
    }
    if (event.key === "o" || event.key === "O") {
      event.preventDefault();
      if (event.shiftKey) {
        store.pushUndoState();
        store.trimEnd = duration;
      } else {
        setTrimPoint("out");
      }
    }

    // Alt+[ trims the IN point one frame later (shrinks from the head);
    // Alt+] trims the OUT point one frame earlier (shrinks from the tail).
    // Shift+Alt+ switches the unit from one frame to one second. We match
    // `event.code` because shifted brackets become "{"/"}" on some layouts.
    if (event.altKey && event.code === "BracketLeft") {
      event.preventDefault();
      nudgeTrim("in", 1, event.shiftKey);
    }
    if (event.altKey && event.code === "BracketRight") {
      event.preventDefault();
      nudgeTrim("out", -1, event.shiftKey);
    }

    // Home/End jump the playhead to the in/out points (NLE convention).
    if (event.key === "Home") {
      event.preventDefault();
      const t = store.inPoint;
      store.currentTime = t;
      if (videoEl) videoEl.currentTime = t;
    }
    if (event.key === "End") {
      event.preventDefault();
      const t = Math.max(store.inPoint, store.outPoint - frameStep());
      store.currentTime = t;
      if (videoEl) videoEl.currentTime = t;
    }

    // J/K/L transport. K parks playback. L plays forward; consecutive Ls
    // step the playback rate up through SHUTTLE_SPEEDS. J does the same in
    // reverse via a rAF-driven loop (browsers don't reliably support
    // negative <video> playbackRate).
    if (event.key === "k" || event.key === "K") {
      event.preventDefault();
      shuttleDirection = 0;
      shuttleSpeedIndex = 0;
      if (videoEl) videoEl.pause();
      store.isPlaying = false;
    }
    if (event.key === "l" || event.key === "L") {
      event.preventDefault();
      if (shuttleDirection === 1) {
        shuttleSpeedIndex = Math.min(
          SHUTTLE_SPEEDS.length - 1,
          shuttleSpeedIndex + 1,
        );
      } else {
        shuttleDirection = 1;
        shuttleSpeedIndex = 0;
      }
      if (videoEl) {
        videoEl.playbackRate =
          SHUTTLE_SPEEDS[shuttleSpeedIndex] * playbackSpeed;
        void videoEl.play();
      }
      store.isPlaying = true;
    }
    if (event.key === "j" || event.key === "J") {
      event.preventDefault();
      if (videoEl) videoEl.pause();
      store.isPlaying = false;
      if (shuttleDirection === -1) {
        shuttleSpeedIndex = Math.min(
          SHUTTLE_SPEEDS.length - 1,
          shuttleSpeedIndex + 1,
        );
      } else {
        shuttleDirection = -1;
        shuttleSpeedIndex = 0;
      }
    }
  }

  function handleResize() {
    if (!timelineEl) return;
    timelineWidth = timelineEl.clientWidth;
  }

  function handleTimelineWheel(event: WheelEvent) {
    if (!timelineEl) return;

    if (event.ctrlKey || event.metaKey) {
      event.preventDefault();
      const rect = timelineEl.getBoundingClientRect();
      const anchorX = event.clientX - rect.left;
      const anchorTime =
        duration > 0 ? (timelineEl.scrollLeft + anchorX) / pixelsPerSecond : 0;
      const delta = event.deltaY < 0 ? 0.2 : -0.2;
      const nextZoom = Math.max(0.5, Math.min(5, store.timelineZoom + delta));
      if (nextZoom === store.timelineZoom) return;
      store.timelineZoom = nextZoom;
      requestAnimationFrame(() => {
        if (!timelineEl || duration <= 0) return;
        const nextPixelsPerSecond =
          (timelineEl.clientWidth * nextZoom) / duration;
        timelineEl.scrollLeft = Math.max(
          0,
          anchorTime * nextPixelsPerSecond - anchorX,
        );
      });
      return;
    }

    if (Math.abs(event.deltaY) > Math.abs(event.deltaX)) {
      event.preventDefault();
      timelineEl.scrollLeft += event.deltaY;
    }
  }

  function syncVideoTime() {
    if (!videoEl) return;
    videoEl.currentTime = Math.max(0, Math.min(duration, store.currentTime));
  }

  function addFocusRegion() {
    if (duration <= 0) return;
    const start = Math.max(store.inPoint, store.currentTime - 0.35);
    const end = Math.min(
      store.outPoint,
      Math.max(start + 0.8, store.currentTime + 0.85),
    );
    store.addZoomRegion(start, end, 1.8);
  }

  // Approximate polynomial-in-t eval; indistinguishable from the real
  // Newton-Raphson solve at sparkline resolution.
  function approxEaseY(easing: Easing, x: number): number {
    const a = 1 - 3 * easing.y2 + 3 * easing.y1;
    const b = 3 * easing.y2 - 6 * easing.y1;
    const c = 3 * easing.y1;
    return ((a * x + b) * x + c) * x;
  }

  // Path drawing 0..100 × 0..18 viewBox: the region's scale curve, normalised
  // so peak scale reaches the top of the box. Shows the rampIn/hold/rampOut
  // shape at a glance.
  function zoomSparklinePath(r: ZoomRegion): string {
    const duration = Math.max(0.001, r.end - r.start);
    const half = duration * 0.5;
    const rampIn = Math.min(Math.max(0, r.rampIn), half);
    const rampOut = Math.min(Math.max(0, r.rampOut), half);
    const holdStart = rampIn;
    const holdEnd = duration - rampOut;
    const peak = Math.max(r.scale, 1.0);
    const norm = (s: number) => (peak === 1 ? 0 : (s - 1) / (peak - 1));
    const W = 100;
    const H = 18;
    const pts: string[] = [];
    const N = 48;
    for (let i = 0; i <= N; i++) {
      const t = (i / N) * duration;
      let s = 1.0;
      if (t < holdStart) {
        const phase = rampIn > 0 ? t / rampIn : 1;
        s = 1 + (r.scale - 1) * approxEaseY(r.easeIn, phase);
      } else if (t > holdEnd) {
        const phase = rampOut > 0 ? (duration - t) / rampOut : 1;
        s = 1 + (r.scale - 1) * approxEaseY(r.easeOut, phase);
      } else {
        s = r.scale;
      }
      const x = (t / duration) * W;
      const y = H - norm(s) * (H - 2) - 1;
      pts.push(`${i === 0 ? "M" : "L"} ${x.toFixed(2)} ${y.toFixed(2)}`);
    }
    return pts.join(" ");
  }

  function setTrimPoint(kind: "in" | "out") {
    if (duration <= 0) return;
    store.pushUndoState();
    const min = minClipDuration();
    if (kind === "in") {
      const nextIn = quantizeToFrame(
        Math.min(store.currentTime, Math.max(0, store.outPoint - min)),
      );
      store.trimStart = nextIn;
      if (store.currentTime < nextIn) store.currentTime = nextIn;
    } else {
      const nextOut = quantizeToFrame(
        Math.max(store.currentTime, Math.min(duration, store.inPoint + min)),
      );
      store.trimEnd = nextOut;
      if (store.currentTime > nextOut) store.currentTime = nextOut;
    }
    syncVideoTime();
  }

  function resetTrim() {
    store.pushUndoState();
    store.trimStart = 0;
    store.trimEnd = duration;
    syncVideoTime();
  }
  function removeZoomRegion(id: string) {
    store.removeZoomRegion(id);
  }

  onMount(() => {
    handleResize();
    const observer = new ResizeObserver(handleResize);
    if (timelineEl) observer.observe(timelineEl);
    return () => observer.disconnect();
  });
</script>

<div
  class="shrink-0 select-none border-t border-border/60 bg-card/30 px-2 pt-1.5 pb-2"
>
  <div
    class="mb-2 flex flex-wrap items-center justify-between gap-2 text-[11px]"
  >
    <div class="flex items-center gap-1">
      <HoverCard>
        <HoverCardTrigger type="button">
          <span
            class="flex size-6 items-center justify-center rounded-md text-muted-foreground/70 transition-colors hover:bg-muted/60 hover:text-foreground"
          >
            <CircleQuestionMark class="size-3.5" />
          </span>
        </HoverCardTrigger>
        <HoverCardContent alignOffset={20}>
          Move the playhead to where you want the clip to begin or end, then
          click <span class="text-foreground">Start here</span> or
          <span class="text-foreground">End here</span>. Anything outside the
          highlighted region is cut from the export.
        </HoverCardContent>
      </HoverCard>

      <!-- Trim segmented pill -->
      <div
        class="flex items-center gap-0.5 rounded-lg bg-muted/60 p-0.5 ring-1 ring-inset ring-border/40"
      >
        <button
          type="button"
          onclick={() => setTrimPoint("in")}
          title="Cut everything before the playhead (I)"
          class="flex h-6 items-center gap-1 rounded-md px-2 text-[11px] font-semibold text-muted-foreground transition-colors duration-150 hover:bg-card hover:text-foreground"
        >
          <span class="hidden sm:inline">Start here</span>
          <span class="sm:hidden">Start</span>
          <Kbd class="ml-0.5">I</Kbd>
        </button>
        <button
          type="button"
          onclick={() => setTrimPoint("out")}
          title="Cut everything after the playhead (O)"
          class="flex h-6 items-center gap-1 rounded-md px-2 text-[11px] font-semibold text-muted-foreground transition-colors duration-150 hover:bg-card hover:text-foreground"
        >
          <span class="hidden sm:inline">End here</span>
          <span class="sm:hidden">End</span>
          <Kbd class="ml-0.5">O</Kbd>
        </button>
      </div>

      <!-- Focus / Suggest pill -->
      <div
        class="flex items-center gap-0.5 rounded-lg bg-muted/60 p-0.5 ring-1 ring-inset ring-border/40"
      >
        <button
          type="button"
          onclick={addFocusRegion}
          class="flex h-6 items-center gap-1 rounded-md px-2 text-[11px] font-semibold text-muted-foreground transition-colors duration-150 hover:bg-card hover:text-foreground"
        >
          <Search class="size-3" />
          Focus
        </button>
        <div class="relative">
          <button
            type="button"
            aria-pressed={showSuggestions}
            onclick={toggleSuggestions}
            disabled={!store.cursorPath}
            title={store.cursorPath
              ? "Suggest focus regions from captured cursor activity"
              : "No cursor data in this clip"}
            class={cn(
              "flex h-6 items-center gap-1 rounded-md px-2 text-[11px] font-semibold transition-colors duration-150 disabled:opacity-40",
              showSuggestions
                ? "bg-card text-foreground shadow-(--shadow-craft-inset) ring-1 ring-inset ring-border/40"
                : "text-muted-foreground hover:bg-card hover:text-foreground",
            )}
          >
            <Wand2 class="size-3" />
            Suggest
          </button>
          {#if showSuggestions}
            <div class="absolute left-0 bottom-full z-40 mt-1.5">
              <ZoomSuggestionsPopover
                {store}
                onclose={() => (showSuggestions = false)}
              />
            </div>
          {/if}
        </div>
      </div>

      {#if hasTrim}
        <button
          type="button"
          onclick={resetTrim}
          title="Restore the full recording — undo all cuts"
          class="flex h-6 items-center gap-1 rounded-md border border-border/40 bg-muted/40 px-2 text-[11px] font-semibold text-muted-foreground transition-colors duration-150 hover:bg-card hover:text-foreground"
        >
          <Scissors class="size-3" />
          Use full clip
        </button>
      {/if}
    </div>

    <div class="flex items-center gap-1.5 text-muted-foreground">
      <!-- Speed menu -->
      <DropdownMenu.Root>
        <DropdownMenu.Trigger>
          <button
            type="button"
            aria-label="Playback speed"
            class="flex h-6 items-center gap-1 rounded-md border border-border/40 bg-muted/40 px-2 font-mono text-[11px] font-semibold tabular-nums text-muted-foreground transition-colors duration-150 hover:bg-card hover:text-foreground"
          >
            <Gauge class="size-3" />
            {playbackSpeed.toFixed(2).replace(/\.?0+$/, "")}×
          </button>
        </DropdownMenu.Trigger>
        <DropdownMenu.Content size="sm" align="end" class="w-24">
          {#each SPEEDS as speed (speed)}
            <DropdownMenu.Item
              onclick={() => (playbackSpeed = speed)}
              class={playbackSpeed === speed ? "text-primary" : ""}
            >
              {speed.toFixed(2).replace(/\.?0+$/, "")}×
            </DropdownMenu.Item>
          {/each}
        </DropdownMenu.Content>
      </DropdownMenu.Root>

      <!-- Zoom segmented pill -->
      <div
        class="flex items-center gap-0.5 rounded-lg bg-muted/60 p-0.5 ring-1 ring-inset ring-border/40"
      >
        <button
          type="button"
          onclick={() => zoomTimeline(-1)}
          aria-label="Zoom out timeline"
          class="flex size-6 items-center justify-center rounded-md text-muted-foreground transition-colors duration-150 hover:bg-card hover:text-foreground"
        >
          <ZoomOut class="size-3" />
        </button>
        <span
          class="min-w-9 text-center font-mono text-[10px] font-semibold tabular-nums text-foreground"
        >
          {store.timelineZoom.toFixed(1)}×
        </span>
        <button
          type="button"
          onclick={() => zoomTimeline(1)}
          aria-label="Zoom in timeline"
          class="flex size-6 items-center justify-center rounded-md text-muted-foreground transition-colors duration-150 hover:bg-card hover:text-foreground"
        >
          <ZoomIn class="size-3" />
        </button>
      </div>

      <!-- Stat chips -->
      <div class="flex items-center gap-1">
        <span
          class="inline-flex h-6 items-center rounded-md border border-border/40 bg-muted/40 px-2 font-mono text-[10px] font-semibold tabular-nums text-foreground"
        >
          {aspectRatioLabel}
        </span>
        <span
          class="inline-flex h-6 items-center rounded-md border border-border/40 bg-muted/40 px-2 font-mono text-[10px] font-semibold tabular-nums text-foreground"
        >
          {frameCount}f
        </span>
        {#if hasTrim}
          <span
            class="inline-flex h-6 items-center rounded-md border border-primary/30 bg-primary/10 px-2 font-mono text-[10px] font-semibold tabular-nums text-primary"
          >
            {formatTimecode(store.clipDuration)}
          </span>
        {/if}
      </div>

      <!-- Kbd hints -->
      <div class="hidden items-center gap-1.5 pl-1 text-[10px] md:flex">
        <span class="inline-flex items-center gap-1">
          <Kbd>Scroll</Kbd>
          <span>pan</span>
        </span>
        <span class="inline-flex items-center gap-1">
          <Kbd>⌘ Scroll</Kbd>
          <span>zoom</span>
        </span>
      </div>
    </div>
  </div>

  <div
    bind:this={timelineEl}
    role="slider"
    tabindex="0"
    aria-label="Timeline scrubber"
    aria-valuemin={0}
    aria-valuemax={duration}
    aria-valuenow={store.currentTime}
    class="custom-scrollbar relative overflow-x-auto overflow-y-hidden rounded-xl border border-border/60 bg-background/60 shadow-(--shadow-craft-inset)"
    onpointerdown={handleTimelinePointerDown}
    onpointermove={handleTimelinePointerMove}
    onpointerup={handleTimelinePointerUp}
    onpointercancel={handleTimelinePointerUp}
    onwheel={handleTimelineWheel}
    onkeydown={handleTimelineKeydown}
  >
    <div
      class="relative min-w-full"
      style="width: {totalWidth}px; height: 156px;"
    >
      <div class="relative h-7 border-b border-border/60 bg-muted/20">
        {#each minorTicks as tick}
          <div
            class="absolute bottom-0 w-px bg-border/50"
            style="left: {tick * pixelsPerSecond}px; height: 5px;"
          ></div>
        {/each}

        {#each timeMarkers as marker}
          <div
            class="absolute top-0 flex h-full flex-col items-start"
            style="left: {marker.time * pixelsPerSecond}px;"
          >
            <div
              class="w-px bg-border"
              style="height: {marker.emphasis ? '10px' : '6px'};"
            ></div>
            <span
              class="mt-0.5 -translate-x-1/2 font-mono tabular-nums text-[10px] text-muted-foreground/80"
            >
              {marker.label}
            </span>
          </div>
        {/each}
      </div>

      <div class="relative px-2 pb-2 pt-1.5">
        <div
          class="relative h-12 rounded-md border border-border/60 bg-background"
        >
          <div
            class="absolute inset-y-0 rounded-md border border-primary/40 bg-primary/5"
            style="left: {clipLeft}px; width: {clipWidth}px;"
          >
            <div class="absolute inset-0 overflow-hidden rounded-md">
              {#if store.thumbnailStrip.length > 0}
                <div class="flex h-full">
                  {#each store.thumbnailStrip as frame, index (frame + index)}
                    <img
                      in:fade={{ duration: 180 }}
                      src={frame}
                      alt="Timeline frame"
                      class="h-full shrink-0 object-cover"
                      style="width: {thumbnailWidth}px;"
                      draggable="false"
                    />
                  {/each}
                </div>
              {:else}
                <div
                  class="flex h-full items-center justify-center text-[10px] text-muted-foreground"
                >
                  Generating thumbnails…
                </div>
              {/if}
            </div>

            <div
              class="absolute left-2 top-1 rounded border border-border bg-background/80 px-1.5 py-0.5 font-mono text-[9px] text-muted-foreground backdrop-blur"
            >
              {hasTrim ? "This part exports" : "Full clip"}
            </div>
            <!--
              Trim drag handles. Each is a narrow vertical bar with a larger
              invisible hit area (±6 px either side) so grabbing is easy.
              Pointer events stop propagation so we don't fight the timeline's
              click-to-seek / playhead-scrub handlers.
            -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              role="slider"
              tabindex="0"
              aria-label="In point"
              aria-valuemin={0}
              aria-valuemax={duration}
              aria-valuenow={store.inPoint}
              aria-valuetext={formatTimecode(store.inPoint)}
              onpointerdown={(e) => startTrimDrag(e, "in")}
              onkeydown={(e) => handleTrimHandleKey(e, "in")}
              class="group absolute inset-y-0 left-0 z-10 w-2 -translate-x-1 cursor-ew-resize focus-visible:outline-none"
            >
              <div class="mx-auto h-full w-1 rounded-l-md bg-primary transition-all group-hover:w-1.5 group-hover:shadow-[0_0_0_2px_rgba(59,130,246,0.3)]"></div>
              {#if activeTrimHandle === "in" && trimDragContext}
                {@const delta = store.inPoint - trimDragContext.originalAt}
                <div
                  class="pointer-events-none absolute bottom-full left-1/2 mb-1 flex -translate-x-1/2 items-center gap-1.5 whitespace-nowrap rounded border border-border bg-popover px-1.5 py-0.5 font-mono text-[9px] text-foreground shadow-sm"
                >
                  <span>In {formatTimecode(store.inPoint)}</span>
                  {#if delta !== 0}
                    <span class="text-muted-foreground"
                      >{delta > 0 ? "+" : ""}{Math.round(delta * effectiveFps())} f</span
                    >
                  {/if}
                </div>
              {/if}
            </div>
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              role="slider"
              tabindex="0"
              aria-label="Out point"
              aria-valuemin={0}
              aria-valuemax={duration}
              aria-valuenow={store.outPoint}
              aria-valuetext={formatTimecode(store.outPoint)}
              onpointerdown={(e) => startTrimDrag(e, "out")}
              onkeydown={(e) => handleTrimHandleKey(e, "out")}
              class="group absolute inset-y-0 right-0 z-10 w-2 translate-x-1 cursor-ew-resize focus-visible:outline-none"
            >
              <div class="mx-auto h-full w-1 rounded-r-md bg-primary transition-all group-hover:w-1.5 group-hover:shadow-[0_0_0_2px_rgba(59,130,246,0.3)]"></div>
              {#if activeTrimHandle === "out" && trimDragContext}
                {@const delta = store.outPoint - trimDragContext.originalAt}
                <div
                  class="pointer-events-none absolute bottom-full left-1/2 mb-1 flex -translate-x-1/2 items-center gap-1.5 whitespace-nowrap rounded border border-border bg-popover px-1.5 py-0.5 font-mono text-[9px] text-foreground shadow-sm"
                >
                  <span>Out {formatTimecode(store.outPoint)}</span>
                  {#if delta !== 0}
                    <span class="text-muted-foreground"
                      >{delta > 0 ? "+" : ""}{Math.round(delta * effectiveFps())} f</span
                    >
                  {/if}
                </div>
              {/if}
            </div>
          </div>
        </div>

        <div
          class="mt-1.5 min-h-9 rounded-md border border-border/60 bg-background/40 px-1.5 py-1.5"
        >
          {#if store.zoomRegions.length === 0}
            <div
              class="flex h-6 items-center justify-center text-[10px] text-muted-foreground"
            >
              Add a focus region to punch in during playback
            </div>
          {:else}
            {#each store.zoomRegions as region, index (region.id)}
              {@const isSelected = region.id === store.selectedZoomRegionId}
              <button
                type="button"
                onclick={(e) => {
                  e.stopPropagation();
                  (store.selectedZoomRegionId = region.id)
                }}
                in:fly={{ y: 10, duration: 180, easing: cubicOut }}
                out:fade={{ duration: 140 }}
                aria-pressed={isSelected}

                class="z-50 absolute overflow-hidden rounded border bg-muted text-left transition-colors focus:outline-none focus:ring-1 focus:ring-ring {isSelected ? 'border-primary ring-1 ring-primary/40' : 'border-border hover:border-primary/60'}"
                style="
									left: {region.start * pixelsPerSecond}px;
									width: {Math.max((region.end - region.start) * pixelsPerSecond, 56)}px;
									top: {86 + index * 2}px;
									height: 30px;
								"
              >
                <svg
                  viewBox="0 0 100 18"
                  preserveAspectRatio="none"
                  class="pointer-events-none absolute inset-x-0 bottom-0 h-3 w-full text-primary/60"
                >
                  <path d={zoomSparklinePath(region)} stroke="currentColor" stroke-width="1.2" fill="none" />
                </svg>
                <div
                  class="relative flex h-full items-center justify-between gap-2 px-2"
                  id={`zoom-region-${region.id}`}
                  aria-label={`Focus region from ${formatTime(region.start)} to ${formatTime(region.end)}, scale ${region.scale.toFixed(1)}x. Click to select, or press Enter or Space when focused. Press the X button to remove.`}
                >
                  <div class="min-w-0">
                    <p class="text-[10px] font-semibold text-foreground">
                      Focus
                    </p>
                    <p class="text-[9px] text-muted-foreground">
                      {region.scale.toFixed(1)}x · {formatTime(region.start)}
                    </p>
                  </div>
                  <span
                    role="button"
                    id={`remove-zoom-region-${region.id}`}
                    tabindex="0"
                    onclick={(event) => {
                      event.stopPropagation();
                      store.removeZoomRegion(region.id);
                    }}
                    onpointerdown={(event) => {
                      event.stopPropagation();
                      store.removeZoomRegion(region.id);
                    }}
                    onkeydown={(event) => {
                      event.preventDefault();
                      event.stopPropagation();
                      if (event.key === "Enter" || event.key === " ") {
                        removeZoomRegion(region.id);
                      }
                    }}
                    class="flex h-4 w-4 shrink-0 cursor-pointer items-center justify-center rounded border border-border bg-background/70 text-muted-foreground transition-colors hover:border-destructive hover:text-destructive pointer-events-auto"
                    aria-label="Remove focus region"
                  >
                    <X size={9} strokeWidth={2.5} />
                  </span>
                </div>
              </button>
            {/each}
          {/if}
        </div>
      </div>

      <div
        class="absolute top-0 z-30 transition-[left] ease-out"
        style="left: {playheadLeft}px; transition-duration: {isDraggingPlayhead
          ? '0ms'
          : '90ms'};"
      >
        <div class="relative -translate-x-1/2">
          <div
            class="absolute left-1/2 top-1 -translate-x-1/2 rounded border border-border bg-foreground px-1.5 py-0.5 font-mono text-[9px] tabular-nums text-background"
          >
            {formatTimecode(store.currentTime)}
          </div>
          <div
            class="mx-auto mt-6 size-2 rounded-full border border-background bg-primary"
          ></div>
          <div class="mx-auto h-31.5 w-px bg-primary/60 pointer-events-none" id="timeline-control"></div>
        </div>
      </div>
    </div>
  </div>
</div>

<style>
  .custom-scrollbar::-webkit-scrollbar {
    height: 8px;
  }

  .custom-scrollbar::-webkit-scrollbar-track {
    background: transparent;
  }

  .custom-scrollbar::-webkit-scrollbar-thumb {
    background: rgba(120, 120, 128, 0.35);
    border-radius: 999px;
  }

  .custom-scrollbar {
    scrollbar-width: thin;
    scrollbar-color: rgba(120, 120, 128, 0.35) transparent;
  }
</style>
