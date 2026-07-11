<script lang="ts">
  import type {
    EditorStore,
    ZoomRegion,
  } from "$lib/stores/editor-store.svelte";
  import { experimentalStore } from "$lib/stores/experimental.svelte";
  import { Film, Pencil, Scissors, Target } from "@lucide/svelte";
  import { onMount } from "svelte";
  import TimelineAnnotationLane from "./_components/timeline/TimelineAnnotationLane.svelte";
  import TimelineClipBar from "./_components/timeline/TimelineClipBar.svelte";
  import TimelineCutLane from "./_components/timeline/TimelineCutLane.svelte";
  import TimelinePlayhead from "./_components/timeline/TimelinePlayhead.svelte";
  import TimelineRuler from "./_components/timeline/TimelineRuler.svelte";
  import TimelineToolbar from "./_components/timeline/TimelineToolbar.svelte";
  import TimelineZoomLane from "./_components/timeline/TimelineZoomLane.svelte";
  import {
    effectiveFps as effFps,
    formatTimeByMode,
    frameStep as frameStepOf,
    greatestCommonDivisor,
    minClipDuration as minClipDurOf,
    quantizeToFrame as quantizeToFrameOf,
    type TimeMode,
  } from "./_components/timeline/timeline-helpers";
  import { originalToOutput, outputToOriginal } from "$lib/timeline/time-map";
  import { buildSnapTargets, snapTime } from "./_components/timeline/timeline-snap";
  import { storyboardCrop } from "$lib/timeline/storyboard";
  import type { TileProvider } from "$lib/timeline/filmstrip-source";

  // Orchestrator: owns the scroll container, sizing, transport (JKL/speed),
  // keyboard routing, and the click-to-seek scrubber. Subviews live under `_components/timeline/`.

  interface Props {
    store: EditorStore;
    videoEl?: HTMLVideoElement | null;
    tileProvider?: TileProvider | null;
    filmstripVersion?: number;
  }

  let {
    store,
    videoEl = null,
    tileProvider = null,
    filmstripVersion = 0,
  }: Props = $props();

  let timelineEl: HTMLDivElement | undefined = $state();
  let isDraggingPlayhead = $state(false);
  let timelineWidth = $state(900);
  // Horizontal scroll offset, tracked so the clip bar can virtualize its tiles.
  let scrollLeft = $state(0);
  // Lane content shares the scroller's x-origin (no left padding), so the clip
  // bar's viewport math needs no offset.
  const LANE_PAD = 0;

  const SPEEDS = [0.25, 0.5, 1.0, 1.5, 2.0] as const;
  let playbackSpeed = $state(1.0);

  // Lives in the orchestrator so one click flips every timeline label at once.
  let timeMode = $state<TimeMode>("smpte");

  // Layer visibility (the toolbar's Layers menu). The clip track is always shown
  // (the editing spine); its content is thumbnails OR the waveform, never both,
  // so they can't overlap. Zoom/Markup lanes show/hide independently. Persisted
  // to localStorage so the choice survives reopening the editor.
  type ClipContent = "thumbnails" | "waveform";
  const VIEW_KEY = "recast.timeline.view";
  function loadView(): {
    clipContent: ClipContent;
    zoom: boolean;
    markup: boolean;
    silence: boolean;
  } {
    if (typeof localStorage !== "undefined") {
      try {
        const raw = localStorage.getItem(VIEW_KEY);
        if (raw) {
          const v = JSON.parse(raw);
          return {
            clipContent: v.clipContent === "waveform" ? "waveform" : "thumbnails",
            zoom: v.zoom !== false,
            markup: v.markup !== false,
            silence: v.silence !== false,
          };
        }
      } catch {
        /* fall through to defaults */
      }
    }
    return { clipContent: "thumbnails", zoom: true, markup: true, silence: true };
  }
  const _view = loadView();
  let clipContent = $state<ClipContent>(_view.clipContent);
  let showZoomLane = $state(_view.zoom);
  let showMarkupLane = $state(_view.markup);
  let showSilenceLane = $state(_view.silence);
  // The Silence lane only exists when the experimental flag is on; this gates
  // both the rail header and the lane so the two never disagree.
  const silenceLaneVisible = $derived(
    experimentalStore.silenceDetection && showSilenceLane,
  );
  $effect(() => {
    if (typeof localStorage === "undefined") return;
    try {
      localStorage.setItem(
        VIEW_KEY,
        JSON.stringify({
          clipContent,
          zoom: showZoomLane,
          markup: showMarkupLane,
          silence: showSilenceLane,
        }),
      );
    } catch {
      /* storage full / unavailable; view prefs are best-effort */
    }
  });

  // JKL transport (Avid/Premiere): consecutive L/J cycles 1×→2×→4×, K parks.
  // J drives reverse via a rAF loop (browsers don't reliably support negative playbackRate).
  let shuttleDirection = $state<-1 | 0 | 1>(0);
  let shuttleSpeedIndex = $state(0);
  const SHUTTLE_SPEEDS = [1, 2, 4];
  let reverseFrame = 0;

  $effect(() => {
    if (!videoEl) return;
    // Legacy <video> path: the element IS the clock, so per-segment clip speed
    // must ride on its playbackRate (the warped output clock only exists on the
    // WebCodecs path). Re-evaluated as the playhead crosses into each segment.
    const segSpeed = store.segmentSpeedAtTime(store.currentTime);
    const transport =
      shuttleDirection === 1
        ? SHUTTLE_SPEEDS[shuttleSpeedIndex] * playbackSpeed
        : playbackSpeed;
    videoEl.playbackRate = transport * segSpeed;
  });

  // Reverse-play loop. Held active only while shuttleDirection === -1.
  function pumpReverse() {
    if (shuttleDirection !== -1 || !videoEl) {
      reverseFrame = 0;
      return;
    }
    const f = effectiveFps();
    const step = (SHUTTLE_SPEEDS[shuttleSpeedIndex] / f) * playbackSpeed;
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

  // Auto-follow: while playing, keep the playhead in view. Only acts once the
  // playhead crosses the leading/trailing margin, so manual scrolling mid-play
  // is left alone until it actually runs off-screen; then we page it back near
  // the left margin. No-op when everything already fits (scrollLeft stays 0).
  $effect(() => {
    if (!store.isPlaying || isDraggingPlayhead || !timelineEl) return;
    const px = xOf(store.currentTime);
    const view = timelineEl.clientWidth;
    const left = timelineEl.scrollLeft;
    const margin = Math.min(view * 0.12, 120);
    if (px < left + margin || px > left + view - margin) {
      timelineEl.scrollLeft = Math.max(0, px - margin);
    }
  });

  // Trim/playhead writes round to the nearest frame so preview and export agree
  // on the first/last kept frame; sub-frame values cause off-by-one mismatches.
  function effectiveFps(): number {
    return effFps(store.metadata?.fps);
  }
  function quantizeToFrame(time: number): number {
    return quantizeToFrameOf(time, effectiveFps());
  }
  function frameStep(): number {
    return frameStepOf(effectiveFps());
  }
  function minClipDuration(): number {
    return minClipDurOf(effectiveFps());
  }

  function zoomTimeline(dir: number) {
    store.timelineZoom = Math.max(
      0.5,
      Math.min(5, store.timelineZoom + dir * 0.25),
    );
  }

  // timelineZoom=1 means "duration spans timelineWidth", so fit is just 1.0.
  function zoomToFit() {
    store.timelineZoom = 1;
    requestAnimationFrame(() => {
      if (timelineEl) timelineEl.scrollLeft = 0;
    });
  }

  // Selected region fills ~70% of the viewport (0.7 leaves context on both sides).
  function zoomToSelection() {
    if (!timelineEl || duration <= 0) return;
    const id = store.selectedZoomRegionId;
    if (!id) return;
    const region = store.zoomRegions.find((r) => r.id === id);
    if (!region) return;
    const span = Math.max(0.001, region.end - region.start);
    const target = (duration / span) * 0.7;
    const nextZoom = Math.max(0.5, Math.min(5, target));
    store.timelineZoom = nextZoom;
    requestAnimationFrame(() => {
      if (!timelineEl || outputDuration <= 0) return;
      const nextPps = (timelineEl.clientWidth * nextZoom) / outputDuration;
      // Center on the region's midpoint in OUTPUT pixels.
      const center = (region.start + region.end) * 0.5;
      timelineEl.scrollLeft = Math.max(
        0,
        originalToOutput(store.timeMap, center) * nextPps - timelineEl.clientWidth * 0.5,
      );
    });
  }

  // Output seconds under the pointer, BEFORE the time-map. Trim drags map this
  // through a map FROZEN at drag-start, so the collapsed clip's left edge (which
  // sits at output 0) isn't a degenerate input.
  function clientXToOutput(clientX: number): number {
    if (!timelineEl || pixelsPerSecond <= 0) return 0;
    const rect = timelineEl.getBoundingClientRect();
    return Math.max(
      0,
      (clientX - rect.left + timelineEl.scrollLeft) / pixelsPerSecond,
    );
  }

  // For the global Alt+[ / Alt+] shortcuts (trim handles have their own arrows in TimelineClipBar).
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
        Math.max(
          store.inPoint + min,
          Math.min(duration, store.outPoint + delta),
        ),
      );
      store.trimEnd = next;
    }
  }

  const duration = $derived(store.metadata?.duration ?? 0);
  // The axis is OUTPUT time via `store.timeMap`: cuts collapse to zero width (NLE
  // ripple) and each kept segment is warped by its per-segment speed. Trimmed
  // head/tail stay as 1x context, so the in/out handles still bracket the clip.
  const outputDuration = $derived(store.timeMap.outputDuration);
  const pixelsPerSecond = $derived(
    outputDuration > 0 ? (timelineWidth * store.timelineZoom) / outputDuration : 100,
  );
  const totalWidth = $derived(
    Math.max(outputDuration * pixelsPerSecond, timelineWidth),
  );
  // Canonical axis transforms: every lane positions with `xOf` and resolves pointers with `tOf`.
  const xOf = (t: number) => originalToOutput(store.timeMap, t) * pixelsPerSecond;
  const tOf = (x: number) => outputToOriginal(store.timeMap, x / pixelsPerSecond);
  const clipLeft = $derived(xOf(store.inPoint));
  const clipRight = $derived(xOf(store.outPoint));
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

  function seekToPosition(clientX: number) {
    if (!timelineEl || duration <= 0) return;
    const rect = timelineEl.getBoundingClientRect();
    const scrollLeft = timelineEl.scrollLeft;
    const x = clientX - rect.left + scrollLeft;
    // OUTPUT px → original time via the cut model (raw `x / pps` would make the playhead trail past each cut).
    const time = Math.max(0, Math.min(duration, tOf(x)));
    store.currentTime = time;
    if (videoEl) videoEl.currentTime = time;
  }

  function handleTimelinePointerDown(event: PointerEvent) {
    // Right/middle button is for the context menu, never seek/razor on it.
    if (event.button !== 0) return;
    // Razor mode owns the click: place an anchor / carve a cut, never seek/drag.
    if (razorActive) {
      event.preventDefault();
      razorClickAt(event.clientX);
      return;
    }
    isDraggingPlayhead = true;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    seekToPosition(event.clientX);
  }

  function handleTimelinePointerMove(event: PointerEvent) {
    updateHover(event.clientX, event.clientY);
    if (!isDraggingPlayhead) return;
    seekToPosition(event.clientX);
  }

  function handleTimelinePointerUp() {
    isDraggingPlayhead = false;
  }

  // Razor (Cut) tool: when armed, the scroller stops seeking and instead takes
  // two clicks to carve a manual cut. The first click sets `razorAnchor`; the
  // second commits `addCut(lo, hi)`. Stays armed for repeated cuts until toggled
  // off or Esc. While armed the cursor is a scissor and a destructive preview
  // band shows the span that will be removed.
  let razorActive = $state(false);
  let razorAnchor = $state<number | null>(null);

  function toggleRazor() {
    razorActive = !razorActive;
    razorAnchor = null;
  }

  // Any other edit action exits the Cut tool, so the armed state always reflects
  // the last action (clicking Split while Cut is armed switches to Split).
  function disarmRazor() {
    razorActive = false;
    razorAnchor = null;
  }

  function splitAtPlayhead() {
    disarmRazor();
    store.splitAt(store.currentTime);
  }

  // Snap a razor point to the playhead, clip in/out, and zoom/markup region
  // edges (falls through to the frame grid otherwise) so cuts land precisely.
  function razorSnap(rawOriginal: number): number {
    const targets = buildSnapTargets({
      playhead: store.currentTime,
      inPoint: store.inPoint,
      outPoint: store.outPoint,
      duration,
      regions: store.zoomRegions,
      annotations: store.annotations,
    });
    const tolerance = pixelsPerSecond > 0 ? 6 / pixelsPerSecond : 0;
    return snapTime(rawOriginal, targets, tolerance, effectiveFps()).time;
  }

  // Original time under the pointer, clamped then snapped to the razor's click
  // resolution (so a cut lands on the same frame preview and export use).
  function clientXToOriginal(clientX: number): number {
    if (!timelineEl) return 0;
    const rect = timelineEl.getBoundingClientRect();
    const x = clientX - rect.left + timelineEl.scrollLeft;
    return razorSnap(Math.max(0, Math.min(duration, tOf(x))));
  }

  function razorClickAt(clientX: number) {
    const t = clientXToOriginal(clientX);
    if (razorAnchor === null) {
      razorAnchor = t;
      return;
    }
    const lo = Math.min(razorAnchor, t);
    const hi = Math.max(razorAnchor, t);
    razorAnchor = null;
    // addCut drops sub-10ms ranges; merge folds it into any neighbour.
    if (store.addCut(lo, hi, "manual")) store.mergeCuts();
  }

  // Hover-scrub: a frame thumbnail (decoded by the filmstrip provider) follows
  // the cursor over the timeline, with the output timecode under it.
  let hover = $state<{
    clientX: number;
    clientY: number;
    top: number;
    outputSec: number;
    originalSec: number;
  } | null>(null);
  // Preferred hover image: a cell from the storyboard sprite (one decode for the
  // whole clip, then every position is an instant CSS crop). The first read also
  // kicks off the build. `previewAt` (per-position decode) is only the fallback
  // shown for the brief moment before the sprite is ready.
  const HOVER_PREVIEW_H = 64;
  const hoverCell = $derived.by(() => {
    void filmstripVersion;
    if (!hover || !tileProvider) return undefined;
    const sb = tileProvider.storyboard();
    if (!sb || sb.count <= 0 || sb.durationSec <= 0 || sb.cellH <= 0) {
      return undefined;
    }
    return { url: sb.url, ...storyboardCrop(sb, hover.originalSec, HOVER_PREVIEW_H) };
  });
  const hoverUrl = $derived.by(() => {
    void filmstripVersion;
    if (!hover || !tileProvider || hoverCell) return undefined;
    return tileProvider.previewAt(hover.originalSec);
  });

  // Snapped end of the live razor span (for the preview band) while armed.
  const razorHoverTime = $derived.by(() => {
    if (!razorActive || !hover) return null;
    return razorSnap(Math.max(0, Math.min(duration, hover.originalSec)));
  });

  function updateHover(clientX: number, clientY = 0) {
    if (!timelineEl || isDraggingPlayhead || duration <= 0) {
      hover = null;
      return;
    }
    const rect = timelineEl.getBoundingClientRect();
    const xInViewport = clientX - rect.left;
    if (xInViewport < 0 || xInViewport > rect.width) {
      hover = null;
      return;
    }
    const outputSec = clientXToOutput(clientX);
    hover = {
      clientX,
      clientY,
      top: rect.top,
      outputSec,
      originalSec: outputToOriginal(store.timeMap, outputSec),
    };
  }
  function clearHover() {
    hover = null;
  }

  function handleTimelineKeydown(event: KeyboardEvent) {
    if (duration <= 0) return;

    const mod = event.ctrlKey || event.metaKey;

    // Razor (Cut) tool: C arms/disarms; Esc cancels a pending anchor, else disarms.
    if (event.key === "Escape" && razorActive) {
      event.preventDefault();
      if (razorAnchor !== null) razorAnchor = null;
      else razorActive = false;
      return;
    }
    if ((event.key === "c" || event.key === "C") && !mod) {
      event.preventDefault();
      toggleRazor();
      return;
    }

    // Paste works anywhere in the timeline (cards own copy/duplicate, so they need focus).
    if (mod && (event.key === "v" || event.key === "V")) {
      if (zoomClipboard) {
        event.preventDefault();
        pasteRegion();
      }
      return;
    }

    // Bail on Ctrl/Cmd so a global combo (⌘K/⌘J/⌘S) doesn't also fire a single-letter transport here.
    if (mod) return;

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

    // Alt+[ shrinks from head, Alt+] from tail (Shift = 1s). Match `event.code`
    // because shifted brackets become "{"/"}" on some layouts.
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

    // Split the clip at the playhead (NLE razor, "S").
    if (event.key === "s" || event.key === "S") {
      event.preventDefault();
      splitAtPlayhead();
    }

    // Ripple-delete the selected clip (or the one under the playhead); store returns the join to land on a kept frame.
    if (event.key === "Delete" || event.key === "Backspace") {
      event.preventDefault();
      disarmRazor();
      const target = store.selectedClipStart ?? store.currentTime;
      const joinAt = store.deleteSegmentAt(target);
      if (joinAt !== null) {
        store.currentTime = joinAt;
        if (videoEl) videoEl.currentTime = joinAt;
      }
    }

    // J/K/L transport (see shuttle state above).
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

  function handleScroll() {
    if (timelineEl) scrollLeft = timelineEl.scrollLeft;
  }

  function handleTimelineWheel(event: WheelEvent) {
    if (!timelineEl) return;

    if (event.ctrlKey || event.metaKey) {
      event.preventDefault();
      const rect = timelineEl.getBoundingClientRect();
      const anchorX = event.clientX - rect.left;
      // Anchor in OUTPUT seconds so the point under the cursor stays put across the zoom.
      const anchorOut =
        duration > 0 ? (timelineEl.scrollLeft + anchorX) / pixelsPerSecond : 0;
      const delta = event.deltaY < 0 ? 0.2 : -0.2;
      const nextZoom = Math.max(0.5, Math.min(5, store.timelineZoom + delta));
      if (nextZoom === store.timelineZoom) return;
      store.timelineZoom = nextZoom;
      requestAnimationFrame(() => {
        if (!timelineEl || outputDuration <= 0) return;
        const nextPixelsPerSecond =
          (timelineEl.clientWidth * nextZoom) / outputDuration;
        timelineEl.scrollLeft = Math.max(
          0,
          anchorOut * nextPixelsPerSecond - anchorX,
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
    disarmRazor();
    const start = Math.max(store.inPoint, store.currentTime - 0.35);
    const end = Math.min(
      store.outPoint,
      Math.max(start + 0.8, store.currentTime + 0.85),
    );
    store.addZoomRegion(start, end, 1.8);
  }

  function setTrimPoint(kind: "in" | "out") {
    if (duration <= 0) return;
    disarmRazor();
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

  // Editable fields only: id/source are regenerated on paste so it never collides with an existing region.
  type ZoomClipboard = Omit<ZoomRegion, "id" | "source">;
  let zoomClipboard = $state<ZoomClipboard | null>(null);

  function snapshotForClipboard(r: ZoomRegion): ZoomClipboard {
    return {
      start: r.start,
      end: r.end,
      scale: r.scale,
      easeIn: { ...r.easeIn },
      easeOut: { ...r.easeOut },
      rampIn: r.rampIn,
      rampOut: r.rampOut,
      centerX: r.centerX,
      centerY: r.centerY,
      motionBlur: r.motionBlur,
    };
  }

  function copyRegion(r: ZoomRegion) {
    zoomClipboard = snapshotForClipboard(r);
  }

  // Place a region at `startAt`, copying the rest from `template`.
  function placeRegion(template: ZoomClipboard, startAt: number) {
    if (duration <= 0) return;
    const span = template.end - template.start;
    const start = Math.max(0, Math.min(duration - span, startAt));
    const end = start + span;
    // addZoomRegion only seeds geometry/scale; layer the rest on so the copy matches the source.
    const id = store.addZoomRegion(start, end, template.scale, {
      x: template.centerX,
      y: template.centerY,
    });
    store.updateZoomRegion(id, {
      easeIn: { ...template.easeIn },
      easeOut: { ...template.easeOut },
      rampIn: template.rampIn,
      rampOut: template.rampOut,
      motionBlur: template.motionBlur,
    });
  }

  function duplicateRegion(r: ZoomRegion) {
    const span = r.end - r.start;
    // Offset by min(0.25s, span) so the copy sits visibly right without overshooting.
    const offset = Math.min(0.25, span);
    placeRegion(snapshotForClipboard(r), r.start + offset);
  }

  // Store nudges the geometry diagonally; we add a +0.25s time shift on top.
  function duplicateAnnotation(
    annotation: import("$lib/stores/editor-store.svelte").Annotation,
  ) {
    if (duration <= 0) return;
    const dup = store.duplicateAnnotation(annotation.id);
    if (!dup) return;
    const span = dup.end - dup.start;
    const offset = Math.min(0.25, span);
    const nextStart = Math.max(0, Math.min(duration - span, dup.start + offset));
    store.updateAnnotation(dup.id, {
      start: nextStart,
      end: nextStart + span,
    });
  }

  function pasteRegion() {
    if (!zoomClipboard) return;
    const span = zoomClipboard.end - zoomClipboard.start;
    placeRegion(zoomClipboard, store.currentTime - span * 0.5);
  }

  function resetTrim() {
    disarmRazor();
    store.pushUndoState();
    store.trimStart = 0;
    store.trimEnd = duration;
    syncVideoTime();
  }

  onMount(() => {
    handleResize();
    const observer = new ResizeObserver(handleResize);
    if (timelineEl) observer.observe(timelineEl);
    return () => observer.disconnect();
  });
</script>

<!-- Track-header chip for the fixed left rail: a square, icon stacked over the
     label, so the rail stays narrow and every row header reads the same. -->
{#snippet railLabel(Icon: typeof Film, label: string, chipClass: string)}
  <span
    class="inline-flex min-h-9 min-w-9 flex-col items-center justify-center gap-0.5 rounded-md px-1 py-1 font-mono text-[7px] font-bold uppercase leading-none tracking-wide {chipClass}"
  >
    <Icon class="size-3.5" />
    {label}
  </span>
{/snippet}

<div
  class="shrink-0 select-none border-t border-border/60 bg-card/30 px-2 pt-1.5 pb-2"
>
  <TimelineToolbar
    {store}
    fps={effectiveFps()}
    {hasTrim}
    {aspectRatioLabel}
    {frameCount}
    {playbackSpeed}
    speeds={SPEEDS}
    {timeMode}
    hasSelectedRegion={!!store.selectedZoomRegionId}
    {razorActive}
    clipContent={clipContent}
    {showZoomLane}
    {showMarkupLane}
    {showSilenceLane}
    onSetTrim={setTrimPoint}
    onSplit={splitAtPlayhead}
    onToggleRazor={toggleRazor}
    onAddFocusRegion={addFocusRegion}
    onResetTrim={resetTrim}
    onZoomTimeline={zoomTimeline}
    onSelectSpeed={(speed) => (playbackSpeed = speed)}
    onSetTimeMode={(mode) => (timeMode = mode)}
    onZoomToFit={zoomToFit}
    onZoomToSelection={zoomToSelection}
    onSetClipContent={(c) => (clipContent = c)}
    onToggleZoomLane={() => (showZoomLane = !showZoomLane)}
    onToggleMarkupLane={() => (showMarkupLane = !showMarkupLane)}
    onToggleSilenceLane={() => (showSilenceLane = !showSilenceLane)}
  />

  <!-- Rail lives OUTSIDE the scroller so lane names never overlap a card at t≈0.
       Row heights mirror the track side (h-7 ruler, h-12 clip, mt-1.5+min-h-9 lanes) so labels align. -->
  <div
    class="relative flex overflow-hidden rounded-xl border border-border/60 bg-background/60 shadow-(--shadow-craft-inset)"
  >
    <div
      class="relative z-10 flex w-16 shrink-0 flex-col border-r border-border/60 bg-card/50"
    >
      <!-- Aligns with the ruler -->
      <div class="h-7 border-b border-border/60"></div>
      <div class="px-1 pb-2 pt-1.5">
        <!-- Headers are centered squares; enable/disable lives in the Layers menu. -->
        <div class="flex h-12 items-center justify-center">
          {@render railLabel(Film, "Clip", "bg-foreground/10 text-foreground/80")}
        </div>
        {#if showZoomLane}
          <div class="mt-1.5 flex min-h-9 items-center justify-center">
            {@render railLabel(Target, "Zoom", "bg-primary/15 text-primary")}
          </div>
        {/if}
        {#if showMarkupLane}
          <div class="mt-1.5 flex min-h-9 items-center justify-center">
            {@render railLabel(Pencil, "Markup", "bg-warning/15 text-warning")}
          </div>
        {/if}
        {#if silenceLaneVisible}
          <div class="mt-1.5 flex min-h-9 items-center justify-center">
            {@render railLabel(Scissors, "Silence", "bg-destructive/15 text-destructive")}
          </div>
        {/if}
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
      class="custom-scrollbar relative min-w-0 flex-1 overflow-x-auto overflow-y-hidden"
      style={razorActive ? "cursor: none" : ""}
      onpointerdown={handleTimelinePointerDown}
      onpointermove={handleTimelinePointerMove}
      onpointerup={handleTimelinePointerUp}
      onpointercancel={handleTimelinePointerUp}
      onwheel={handleTimelineWheel}
      onscroll={handleScroll}
      onpointerleave={clearHover}
      onkeydown={handleTimelineKeydown}
    >
      <div class="relative min-w-full" style="width: {totalWidth}px;">
        <TimelineRuler duration={outputDuration} {pixelsPerSecond} />

      <!-- No horizontal padding: lanes must share the x-origin of the ruler and
           playhead (both direct children at x=0), or every tile sits offset from
           the ticks and the playhead line. -->
      <div class="relative pb-2 pt-1.5">
        <TimelineClipBar
          {store}
          {videoEl}
          fps={effectiveFps()}
          {duration}
          {pixelsPerSecond}
          {clipLeft}
          {clipWidth}
          {thumbnailWidth}
          {timeMode}
          content={clipContent}
          {clientXToOutput}
          {tileProvider}
          {filmstripVersion}
          viewportLeftPx={Math.max(0, scrollLeft - LANE_PAD)}
          viewportWidthPx={timelineWidth}
        />

        {#if showZoomLane}
          <TimelineZoomLane
            {store}
            {pixelsPerSecond}
            fps={effectiveFps()}
            {duration}
            {timeMode}
            onCopy={copyRegion}
            onDuplicate={duplicateRegion}
          />
        {/if}

        {#if showMarkupLane}
          <TimelineAnnotationLane
            {store}
            {pixelsPerSecond}
            fps={effectiveFps()}
            {duration}
            {timeMode}
            onDuplicate={duplicateAnnotation}
          />
        {/if}

        {#if silenceLaneVisible}
          <TimelineCutLane {store} {pixelsPerSecond} {duration} />
        {/if}
      </div>

      <TimelinePlayhead
        currentTime={store.currentTime}
        leftPx={xOf(store.currentTime)}
        fps={effectiveFps()}
        isDragging={isDraggingPlayhead}
        {timeMode}
      />

      <!-- Razor preview: a hairline at the pending click point, and once an anchor
           is set, the destructive span that the second click will remove. -->
      {#if razorActive && hover}
        {@const endT = razorHoverTime ?? hover.originalSec}
        {@const anchorX = razorAnchor !== null ? xOf(razorAnchor) : xOf(endT)}
        {@const hoverX = xOf(endT)}
        {#if razorAnchor !== null}
          {@const left = Math.min(anchorX, hoverX)}
          {@const w = Math.abs(hoverX - anchorX)}
          <div
            class="pointer-events-none absolute inset-y-0 z-20 border-x border-destructive/70 bg-destructive/15"
            style="left: {left}px; width: {w}px; background-image: repeating-linear-gradient(45deg, transparent, transparent 5px, color-mix(in srgb, var(--destructive) 20%, transparent) 5px, color-mix(in srgb, var(--destructive) 20%, transparent) 10px);"
          >
            {#if w > 36}
              <span
                class="absolute left-1/2 top-1 -translate-x-1/2 whitespace-nowrap rounded bg-destructive px-1 py-0.5 font-mono text-[9px] font-bold text-destructive-foreground shadow-sm"
              >
                −{Math.abs(endT - razorAnchor).toFixed(2)}s
              </span>
            {/if}
          </div>
        {/if}
        <div
          class="pointer-events-none absolute inset-y-0 z-20 w-px bg-destructive"
          style="left: {anchorX}px;"
        ></div>
      {/if}
      </div>
    </div>
  </div>
</div>

<!-- Scissor cursor for the razor tool: the scroller hides its native cursor
     (cursor:none) and this glyph rides the pointer instead, so the cursor
     literally reads as a scissor while armed. -->
{#if razorActive && hover}
  <div
    class="pointer-events-none fixed z-50 -translate-x-1/2 -translate-y-1/2 text-destructive drop-shadow-md"
    style="left: {hover.clientX}px; top: {hover.clientY}px;"
  >
    <Scissors class="size-5" />
  </div>
{/if}

<!-- Hover-scrub preview: fixed so it floats above the timeline without being
     clipped by the scroller's overflow. Only with the WebCodecs filmstrip. -->
{#if hover && tileProvider && !isDraggingPlayhead && !razorActive}
  <div
    class="pointer-events-none fixed z-50 flex -translate-x-1/2 -translate-y-full flex-col items-center gap-1"
    style="left: {hover.clientX}px; top: {hover.top - 8}px;"
  >
    <div
      class="overflow-hidden rounded-md border border-border/70 bg-card shadow-lg"
    >
      {#if hoverCell}
        <!-- One cell of the storyboard sprite, cropped via background-position. -->
        <div
          class="h-16"
          style="width: {hoverCell.dispW}px; background-image: url('{hoverCell.url}'); background-repeat: no-repeat; background-size: {hoverCell.bgW}px {hoverCell.bgH}px; background-position: -{hoverCell.offX}px -{hoverCell.offY}px;"
        ></div>
      {:else if hoverUrl}
        <img
          src={hoverUrl}
          alt=""
          class="block h-16 w-auto object-cover"
          draggable="false"
        />
      {:else}
        <div class="h-16 w-28 animate-pulse bg-muted/60"></div>
      {/if}
    </div>
    <span
      class="rounded bg-popover px-1.5 py-0.5 font-mono text-[10px] tabular-nums text-foreground shadow-sm"
    >
      {formatTimeByMode(hover.outputSec, timeMode, effectiveFps())}
    </span>
  </div>
{/if}

<style>
  .custom-scrollbar::-webkit-scrollbar {
    height: 8px;
  }

  .custom-scrollbar::-webkit-scrollbar-track {
    background: transparent;
  }

  .custom-scrollbar::-webkit-scrollbar-thumb {
    background: color-mix(in srgb, var(--color-foreground) 14%, transparent);
    border-radius: 999px;
    transition: background 0.2s cubic-bezier(0.625, 0.05, 0, 1);
  }

  .custom-scrollbar::-webkit-scrollbar-thumb:hover {
    background: color-mix(in srgb, var(--color-foreground) 24%, transparent);
  }

  .custom-scrollbar {
    scrollbar-width: thin;
    scrollbar-color: color-mix(in srgb, var(--color-foreground) 14%, transparent)
      transparent;
  }
</style>
