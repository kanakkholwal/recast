<script lang="ts">
  import type {
    EditorStore,
    ZoomRegion,
  } from "$lib/stores/editor-store.svelte";
  import {
    AudioLines,
    Mic,
    Music2,
    Pencil,
    Scissors,
    Video,
    ZoomIn,
  } from "@recast/icons";
  import { onMount } from "svelte";
  import TimelineAnnotationLane from "./_components/timeline/TimelineAnnotationLane.svelte";
  import TimelineAudioLane from "./_components/timeline/TimelineAudioLane.svelte";
  import TimelineMusicLane from "./_components/timeline/TimelineMusicLane.svelte";
  import TimelineClipBar from "./_components/timeline/TimelineClipBar.svelte";
  import TimelineCutLane from "./_components/timeline/TimelineCutLane.svelte";
  import TimelinePlayhead from "./_components/timeline/TimelinePlayhead.svelte";
  import TimelineRuler from "./_components/timeline/TimelineRuler.svelte";
  import TimelineToolbar from "./_components/timeline/TimelineToolbar.svelte";
  import TimelineZoomLane from "./_components/timeline/TimelineZoomLane.svelte";
  import {
    clampTimelineZoom,
    effectiveFps as effFps,
    formatTimeByMode,
    frameStep as frameStepOf,
    greatestCommonDivisor,
    MIN_TIMELINE_ZOOM,
    minClipDuration as minClipDurOf,
    quantizeToFrame as quantizeToFrameOf,
    steppedZoom,
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

  // Lives in the store, not here: the transport readout under the video reads it
  // too, so one setting flips every timecode in the editor at once.
  const timeMode = $derived(store.timeMode);

  // Layer visibility (the toolbar's Layers menu). The clip track is always shown
  // (the editing spine); the waveform now rides ALONG its bottom edge rather than
  // replacing the thumbnails, so it's an independent toggle, not a radio. Zoom/
  // Markup/Cuts lanes show/hide independently. Persisted to localStorage so the
  // choice survives reopening the editor.
  const VIEW_KEY = "recast.timeline.view";
  function loadView(): {
    waveform: boolean;
    zoom: boolean;
    markup: boolean;
    cuts: boolean;
    gaps: boolean;
  } {
    if (typeof localStorage !== "undefined") {
      try {
        const raw = localStorage.getItem(VIEW_KEY);
        if (raw) {
          const v = JSON.parse(raw);
          return {
            // Migrate the old `clipContent: "waveform" | "thumbnails"` radio: anyone
            // who had chosen the waveform still wants to see it, now as an overlay.
            waveform:
              typeof v.waveform === "boolean"
                ? v.waveform
                : v.clipContent === "waveform",
            zoom: v.zoom !== false,
            markup: v.markup !== false,
            cuts: v.cuts ?? v.silence ?? true,
            gaps: v.gaps === true,
          };
        }
      } catch {
        /* fall through to defaults */
      }
    }
    return { waveform: true, zoom: true, markup: true, cuts: true, gaps: false };
  }
  const _view = loadView();
  let showAudioLane = $state(_view.waveform);
  let showZoomLane = $state(_view.zoom);
  let showMarkupLane = $state(_view.markup);
  let showCutLane = $state(_view.cuts);
  // "Show cut gaps" lives in the store (it reshapes the render axis every lane
  // reads); seed it from the persisted view pref on mount.
  onMount(() => {
    store.showCutGaps = _view.gaps;
  });
  $effect(() => {
    if (typeof localStorage === "undefined") return;
    try {
      localStorage.setItem(
        VIEW_KEY,
        JSON.stringify({
          waveform: showAudioLane,
          zoom: showZoomLane,
          markup: showMarkupLane,
          cuts: showCutLane,
          gaps: store.showCutGaps,
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
    store.timelineZoom = steppedZoom(
      store.timelineZoom,
      dir,
      outputDuration,
      timelineWidth,
    );
  }

  // timelineZoom=1 means "duration spans timelineWidth", so fit is just 1.0.
  function zoomToFit() {
    store.timelineZoom = MIN_TIMELINE_ZOOM;
    requestAnimationFrame(() => {
      if (timelineEl) timelineEl.scrollLeft = 0;
    });
  }

  // The [start, end] of whatever is selected, in original time, or null. Drives
  // Zoom-to-selection for any timed selection (zoom region, annotation, cut), not
  // just a focus region. A clip selection has no meaningful frame-to (it is the
  // spine), so it returns null.
  function selectionSpan(): { start: number; end: number } | null {
    const sel = store.selection;
    if (!sel) return null;
    if (sel.kind === "zoom") {
      const r = store.zoomRegions.find((r) => r.id === sel.id);
      return r ? { start: r.start, end: r.end } : null;
    }
    if (sel.kind === "annotation") {
      const a = store.annotations.find((a) => a.id === sel.id);
      return a ? { start: a.start, end: a.end } : null;
    }
    if (sel.kind === "cut") {
      const c = store.cuts.find((c) => c.id === sel.id);
      return c ? { start: c.start, end: c.end } : null;
    }
    return null;
  }
  const hasFramableSelection = $derived(selectionSpan() !== null);

  // Selection fills ~70% of the viewport (0.7 leaves context on both sides).
  function zoomToSelection() {
    if (!timelineEl || duration <= 0) return;
    const span = selectionSpan();
    if (!span) return;
    const width = Math.max(0.001, span.end - span.start);
    const target = (duration / width) * 0.7;
    const nextZoom = clampTimelineZoom(target, outputDuration, timelineWidth);
    store.timelineZoom = nextZoom;
    requestAnimationFrame(() => {
      if (!timelineEl || outputDuration <= 0) return;
      const nextPps = (timelineEl.clientWidth * nextZoom) / outputDuration;
      // Center on the selection's midpoint in OUTPUT pixels.
      const center = (span.start + span.end) * 0.5;
      timelineEl.scrollLeft = Math.max(
        0,
        originalToOutput(store.renderMap, center) * nextPps - timelineEl.clientWidth * 0.5,
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
  // The axis lanes render on: `store.renderMap`. Normally OUTPUT time (cuts
  // collapse to zero width, each kept segment warped by its speed); with "Show cut
  // gaps" on, cuts get real width instead. Playback/export stay on the collapsed map.
  const outputDuration = $derived(store.renderMap.outputDuration);
  const pixelsPerSecond = $derived(
    outputDuration > 0 ? (timelineWidth * store.timelineZoom) / outputDuration : 100,
  );
  const totalWidth = $derived(
    Math.max(outputDuration * pixelsPerSecond, timelineWidth),
  );
  // Canonical axis transforms: every lane positions with `xOf` and resolves pointers with `tOf`.
  const xOf = (t: number) => originalToOutput(store.renderMap, t) * pixelsPerSecond;
  const tOf = (x: number) => outputToOriginal(store.renderMap, x / pixelsPerSecond);
  // The playhead reads on the OUTPUT axis, same as the ruler beneath it and the
  // transport readout above it. Showing `store.currentTime` (original time) here
  // made the chip disagree with the ruler it sits on the moment a cut existed.
  const playheadOutput = $derived(
    originalToOutput(store.renderMap, store.currentTime),
  );
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
    // Right/middle button is for the context menu, never seek on it.
    if (event.button !== 0) return;
    // Razor mode owns the click: place an anchor / carve a cut, never seek/drag.
    if (razorActive) {
      event.preventDefault();
      razorClickAt(event.clientX);
      return;
    }
    // Clicking bare timeline deselects. Cards stop propagation, but clip blocks
    // deliberately don't (the click has to seek too), so they mark themselves
    // `data-selectable` and we leave their selection alone.
    if (!(event.target as HTMLElement).closest("[data-selectable]")) {
      store.clearSelection();
    }
    isDraggingPlayhead = true;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    seekToPosition(event.clientX);
  }

  // Coalesce pointer moves to one rAF: hover + drag-seek each did a
  // getBoundingClientRect (a forced layout), and a drag also fanned out the full
  // `store.currentTime` write — synchronously, per event. High-Hz mice fire well
  // above 60/s, so this bounds that work to once per frame with no perceptible
  // scrub lag.
  let pendingPointer: { x: number; y: number } | null = null;
  let pointerRaf: number | null = null;
  function processPointer() {
    pointerRaf = null;
    const p = pendingPointer;
    if (!p) return;
    updateHover(p.x, p.y);
    if (isDraggingPlayhead) seekToPosition(p.x);
  }
  function handleTimelinePointerMove(event: PointerEvent) {
    pendingPointer = { x: event.clientX, y: event.clientY };
    if (pointerRaf === null) pointerRaf = requestAnimationFrame(processPointer);
  }

  function handleTimelinePointerUp() {
    // Land the final position immediately — the last queued rAF may be up to a
    // frame stale, and a scrub must end exactly where the pointer was released.
    if (pointerRaf !== null) {
      cancelAnimationFrame(pointerRaf);
      pointerRaf = null;
    }
    if (pendingPointer && isDraggingPlayhead) seekToPosition(pendingPointer.x);
    pendingPointer = null;
    isDraggingPlayhead = false;
  }

  // Razor (Cut) tool: when armed, the scroller stops seeking and instead takes
  // two clicks to carve a manual cut. The first click sets `razorAnchor`; the
  // second commits `addCut(lo, hi)`. Stays armed for repeated cuts until toggled
  // off or Esc. While armed the cursor is a scissor and a destructive preview
  // band shows the span that will be removed.
  // The tool lives in the store, not here: a tool is a mode of the whole
  // timeline, and every lane needs to read it to decline the gesture the tool
  // owns (else a razor click over the Cuts/Zoom lane starts a create-drag
  // instead of carving). Local state couldn't reach them.
  const razorActive = $derived(store.timelineTool === "razor");
  let razorAnchor = $state<number | null>(null);

  function toggleRazor() {
    store.timelineTool = razorActive ? "select" : "razor";
    razorAnchor = null;
  }

  // Any other edit action exits the Cut tool, so the armed state always reflects
  // the last action (clicking Split while Cut is armed switches to Split).
  function disarmRazor() {
    store.timelineTool = "select";
    razorAnchor = null;
  }

  // Esc: cancel a pending anchor first, then disarm. Registered so the route can
  // exit the tool even when the scroller never held focus.
  function exitTool() {
    if (razorAnchor !== null) razorAnchor = null;
    else disarmRazor();
  }

  // Jump the playhead to the in/out point (Home/End). Extracted so the route can
  // drive it without the scroller holding focus.
  function seekToEdge(which: "in" | "out") {
    if (duration <= 0) return;
    const t =
      which === "in"
        ? store.inPoint
        : Math.max(store.inPoint, store.outPoint - frameStep());
    store.currentTime = t;
    if (videoEl) videoEl.currentTime = t;
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

  // Last-resort hover frame: the nearest frame of the coarse Rust strip. The
  // WebCodecs sprite/tiles are better, but when the decoder yields nothing the
  // preview used to sit there as an empty grey box. A coarse frame beats none.
  const hoverStripUrl = $derived.by(() => {
    if (!hover || hoverCell || hoverUrl) return undefined;
    const strip = store.thumbnailStrip;
    if (strip.length === 0 || duration <= 0) return undefined;
    const i = Math.min(
      strip.length - 1,
      Math.max(0, Math.floor((hover.originalSec / duration) * strip.length)),
    );
    return strip[i];
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
      originalSec: outputToOriginal(store.renderMap, outputSec),
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
      exitTool();
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
      seekToEdge("in");
    }
    if (event.key === "End") {
      event.preventDefault();
      seekToEdge("out");
    }

    // Split the clip at the playhead ("S").
    if (event.key === "s" || event.key === "S") {
      event.preventDefault();
      splitAtPlayhead();
    }

    // Delete is NOT handled here. It's a document-level command over the current
    // selection, owned by the editor page: three handlers used to claim it (this
    // one, the zoom card, the annotation overlay) and resolve against DOM focus
    // instead of the selection, so it could destroy the object you weren't
    // looking at, or two objects at once.

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

  // The zoom ceiling depends on clip length and viewport width, so a persisted
  // zoom (or a window resize) can land outside the legal range. Pull it back.
  $effect(() => {
    if (outputDuration <= 0 || timelineWidth <= 0) return;
    const legal = clampTimelineZoom(
      store.timelineZoom,
      outputDuration,
      timelineWidth,
    );
    if (legal !== store.timelineZoom) store.timelineZoom = legal;
  });

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
      // Multiplicative, so one wheel notch covers the same proportion of the
      // range whether the clip is 10 seconds or 30 minutes long.
      const nextZoom = clampTimelineZoom(
        store.timelineZoom * (event.deltaY < 0 ? 1.12 : 1 / 1.12),
        outputDuration,
        timelineWidth,
      );
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
    // The route-level keyboard handler drives these so the toolbar's S/C/I/O
    // keycaps are honest whether or not the scroller holds focus. Unregistered
    // on unmount, so they no-op while the timeline is collapsed.
    const offCommands = store.registerTimelineCommands({
      splitAtPlayhead,
      toggleRazor,
      exitTool,
      trimToPlayhead: setTrimPoint,
      seekToEdge,
    });
    return () => {
      observer.disconnect();
      offCommands();
      if (pointerRaf !== null) cancelAnimationFrame(pointerRaf);
    };
  });
</script>

<!-- Track-header chip for the fixed left rail: a square, icon stacked over the
     label, so the rail stays narrow and every row header reads the same. -->
{#snippet railLabel(Icon: typeof Video, label: string, iconClass: string)}
  <span
    class="inline-flex min-h-9 min-w-9 flex-col items-center justify-center gap-0.5 rounded-md bg-muted/60 px-1 py-1 font-mono text-[7px] font-bold uppercase leading-none tracking-wide text-muted-foreground ring-1 ring-inset ring-border/40"
  >
    <Icon class="size-3.5 {iconClass}" />
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
    hasSelectedRegion={hasFramableSelection}
    {razorActive}
    {showAudioLane}
    {showZoomLane}
    {showMarkupLane}
    {showCutLane}
    showCutGaps={store.showCutGaps}
    onSetTrim={setTrimPoint}
    onSplit={splitAtPlayhead}
    onToggleRazor={toggleRazor}
    onAddFocusRegion={addFocusRegion}
    onResetTrim={resetTrim}
    onZoomTimeline={zoomTimeline}
    onSelectSpeed={(speed) => (playbackSpeed = speed)}
    onSetTimeMode={(mode) => (store.timeMode = mode)}
    onZoomToFit={zoomToFit}
    onZoomToSelection={zoomToSelection}
    onToggleAudioLane={() => (showAudioLane = !showAudioLane)}
    onToggleZoomLane={() => (showZoomLane = !showZoomLane)}
    onToggleMarkupLane={() => (showMarkupLane = !showMarkupLane)}
    onToggleCutLane={() => (showCutLane = !showCutLane)}
    onToggleCutGaps={() => (store.showCutGaps = !store.showCutGaps)}
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
          {@render railLabel(Video, "Clip", "text-foreground/70")}
        </div>
        {#if showAudioLane}
          <div class="mt-1.5 flex h-9 items-center justify-center">
            {@render railLabel(AudioLines, "Audio", "text-lane-audio")}
          </div>
        {/if}
        {#if store.voiceClips.length > 0}
          <div class="mt-1.5 flex h-7 items-center justify-center">
            {@render railLabel(Mic, "Voice", "text-lane-audio")}
          </div>
        {/if}
        {#if store.musicOnlyClips.length > 0}
          <div class="mt-1.5 flex h-7 items-center justify-center">
            {@render railLabel(Music2, "Music", "text-lane-music")}
          </div>
        {/if}
        {#if showCutLane}
          <div class="mt-1.5 flex min-h-9 items-center justify-center">
            {@render railLabel(Scissors, "Cuts", "text-lane-cut")}
          </div>
        {/if}
        {#if showZoomLane}
          <div class="mt-1.5 flex min-h-9 items-center justify-center">
            {@render railLabel(ZoomIn, "Zoom", "text-lane-zoom")}
          </div>
        {/if}
        {#if showMarkupLane}
          <div class="mt-1.5 flex min-h-9 items-center justify-center">
            {@render railLabel(Pencil, "Markup", "text-lane-markup")}
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
      class="custom-scrollbar relative min-w-0 flex-1 overflow-x-auto overflow-y-hidden rounded-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/60"
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
        <TimelineRuler
          duration={outputDuration}
          {pixelsPerSecond}
          {timeMode}
          fps={effectiveFps()}
        />

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
          {clientXToOutput}
          {tileProvider}
          {filmstripVersion}
          viewportLeftPx={Math.max(0, scrollLeft - LANE_PAD)}
          viewportWidthPx={timelineWidth}
        />

        {#if showAudioLane}
          <TimelineAudioLane {store} {pixelsPerSecond} {duration} />
        {/if}

        <!-- Detached recording audio ("voice"), then music: both editable clip
             lanes, appearing whenever they hold clips (not just in the panel). -->
        {#if store.voiceClips.length > 0}
          <TimelineMusicLane
            {store}
            clips={store.voiceClips}
            {pixelsPerSecond}
            variant="voice"
            panelTab="audio"
          />
        {/if}
        {#if store.musicOnlyClips.length > 0}
          <TimelineMusicLane
            {store}
            clips={store.musicOnlyClips}
            {pixelsPerSecond}
            variant="music"
            panelTab="music"
          />
        {/if}

        <!-- Cuts sit next to Audio: cutting against the waveform is the common
             task. The cut lane draws its own faint waveform only when the Audio
             lane is hidden, so the two are never stacked as a duplicate. -->
        {#if showCutLane}
          <TimelineCutLane
            {store}
            {pixelsPerSecond}
            {duration}
            showWaveform={!showAudioLane}
          />
        {/if}

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
      </div>

      <TimelinePlayhead
        outputTime={playheadOutput}
        leftPx={playheadOutput * pixelsPerSecond}
        fps={effectiveFps()}
        isDragging={isDraggingPlayhead}
        isPlaying={store.isPlaying}
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
            class="pointer-events-none absolute inset-y-0 z-20 border-x border-lane-cut/70 bg-lane-cut/15"
            style="left: {left}px; width: {w}px; background-image: repeating-linear-gradient(45deg, transparent, transparent 5px, color-mix(in srgb, var(--lane-cut) 20%, transparent) 5px, color-mix(in srgb, var(--lane-cut) 20%, transparent) 10px);"
          >
            {#if w > 36}
              <span
                class="absolute left-1/2 top-1 -translate-x-1/2 whitespace-nowrap rounded bg-lane-cut px-1 py-0.5 font-mono text-[9px] font-bold text-background shadow-sm"
              >
                âˆ’{Math.abs(endT - razorAnchor).toFixed(2)}s
              </span>
            {/if}
          </div>
        {/if}
        <div
          class="pointer-events-none absolute inset-y-0 z-20 w-px bg-lane-cut"
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
    class="pointer-events-none fixed z-50 -translate-x-1/2 -translate-y-1/2 text-lane-cut drop-shadow-md"
    style="left: {hover.clientX}px; top: {hover.clientY}px;"
  >
    <Scissors class="size-5" />
  </div>
{/if}

<!-- Hover-scrub preview: fixed so it floats above the timeline without being
     clipped by the scroller's overflow. Only with the WebCodecs filmstrip. -->
{#if hover && !isDraggingPlayhead && !razorActive && (tileProvider || hoverStripUrl)}
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
      {:else if hoverStripUrl}
        <img
          src={hoverStripUrl}
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
