<script lang="ts">
  import type {
    EditorStore,
    ZoomRegion,
  } from "$lib/stores/editor-store.svelte";
  import { experimentalStore } from "$lib/stores/experimental.svelte";
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
    frameStep as frameStepOf,
    greatestCommonDivisor,
    minClipDuration as minClipDurOf,
    quantizeToFrame as quantizeToFrameOf,
    type TimeMode,
  } from "./_components/timeline/timeline-helpers";

  // Orchestrator. Owns the scroll container, sizing, transport state
  // (JKL/playback speed), keyboard routing, and the click-to-seek scrubber.
  // Visual subviews (toolbar, ruler, clip bar, zoom lane, playhead) live
  // under `_components/timeline/` and receive only the data they render.

  interface Props {
    store: EditorStore;
    videoEl?: HTMLVideoElement | null;
  }

  let { store, videoEl = null }: Props = $props();

  let timelineEl: HTMLDivElement | undefined = $state();
  let isDraggingPlayhead = $state(false);
  let showSuggestions = $state(false);
  let timelineWidth = $state(900);

  const SPEEDS = [0.25, 0.5, 1.0, 1.5, 2.0] as const;
  let playbackSpeed = $state(1.0);

  // Time-display mode. Cycles smpte → seconds → frames; affects every
  // user-visible label (toolbar chip, playhead pill, trim tooltips, card
  // subtitles). Lives in the orchestrator so a single click flips the
  // entire timeline at once.
  const TIME_MODE_CYCLE: readonly TimeMode[] = ["smpte", "seconds", "frames"];
  let timeMode = $state<TimeMode>("smpte");
  function cycleTimeMode() {
    const i = TIME_MODE_CYCLE.indexOf(timeMode);
    timeMode = TIME_MODE_CYCLE[(i + 1) % TIME_MODE_CYCLE.length];
  }

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

  // Quantization: all trim and playhead writes round to the nearest frame
  // boundary so preview and export agree on which exact frame is the
  // first/last kept frame. Sub-frame trim values are the source of off-by-one
  // mismatches between scrub preview and the rendered MP4.
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

  // Zoom so the entire clip fills the visible viewport, then scroll back
  // to the head. timelineZoom=1 means "duration spans timelineWidth", so
  // fit is just `1.0` modulo the rare case where the user has dragged the
  // panel narrower than the rendered clip.
  function zoomToFit() {
    store.timelineZoom = 1;
    requestAnimationFrame(() => {
      if (timelineEl) timelineEl.scrollLeft = 0;
    });
  }

  // Zoom so the selected region fills ~70% of the viewport and centers
  // it horizontally. `0.7` leaves visual breathing room on both sides so
  // neighbouring context isn't lost the moment the user clicks the icon.
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
      if (!timelineEl || duration <= 0) return;
      const nextPps = (timelineEl.clientWidth * nextZoom) / duration;
      const center = (region.start + region.end) * 0.5;
      timelineEl.scrollLeft = Math.max(
        0,
        center * nextPps - timelineEl.clientWidth * 0.5,
      );
    });
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

  // Shared trim-nudge for the global Alt+[ / Alt+] shortcuts. The trim
  // handles use their own ArrowLeft/Right inside TimelineClipBar.
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
  const pixelsPerSecond = $derived(
    duration > 0 ? (timelineWidth * store.timelineZoom) / duration : 100,
  );
  const totalWidth = $derived(
    Math.max(duration * pixelsPerSecond, timelineWidth),
  );
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
    // Cmd/Ctrl + V pastes a previously-copied region at the playhead. Cards
    // own copy/duplicate themselves (those need a focused card), but paste
    // works anywhere in the timeline scope.
    if ((event.ctrlKey || event.metaKey) && (event.key === "v" || event.key === "V")) {
      if (zoomClipboard) {
        event.preventDefault();
        pasteRegion();
        return;
      }
    }

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

  // Internal clipboard for zoom regions. Plain object holding the editable
  // fields of a region — id and source are regenerated on paste so a paste
  // never collides with an existing region's identity.
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

  // Place a region anchored at the supplied start time, copying everything
  // else from `template`. Caller is responsible for clamping start so the
  // span fits inside [0, duration].
  function placeRegion(template: ZoomClipboard, startAt: number) {
    if (duration <= 0) return;
    const span = template.end - template.start;
    const start = Math.max(0, Math.min(duration - span, startAt));
    const end = start + span;
    // store.addZoomRegion only seeds the geometry/scale; layer the rest of
    // the template on with a single follow-up update so the new region
    // is identical to the source modulo position.
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
    // Offset by 0.25s or one full span — whichever is smaller — so the new
    // card sits visibly to the right without overshooting the timeline.
    const offset = Math.min(0.25, span);
    placeRegion(snapshotForClipboard(r), r.start + offset);
  }

  // Duplicate an annotation along with a +0.25s time offset so the copy
  // sits visibly to the right of the source on the timeline. The store
  // already nudges the geometry diagonally; we layer the time shift on top.
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

<div
  class="shrink-0 select-none border-t border-border/60 bg-card/30 px-2 pt-1.5 pb-2"
>
  <TimelineToolbar
    {store}
    fps={effectiveFps()}
    {hasTrim}
    {aspectRatioLabel}
    {frameCount}
    {showSuggestions}
    {playbackSpeed}
    speeds={SPEEDS}
    {timeMode}
    hasSelectedRegion={!!store.selectedZoomRegionId}
    onSetTrim={setTrimPoint}
    onAddFocusRegion={addFocusRegion}
    onToggleSuggestions={toggleSuggestions}
    onCloseSuggestions={() => (showSuggestions = false)}
    onResetTrim={resetTrim}
    onZoomTimeline={zoomTimeline}
    onSelectSpeed={(speed) => (playbackSpeed = speed)}
    onCycleTimeMode={cycleTimeMode}
    onZoomToFit={zoomToFit}
    onZoomToSelection={zoomToSelection}
  />

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
      style="width: {totalWidth}px; height: {experimentalStore.silenceDetection ? 250 : 204}px;"
    >
      <TimelineRuler {duration} {pixelsPerSecond} />

      <div class="relative px-2 pb-2 pt-1.5">
        <TimelineClipBar
          {store}
          {videoEl}
          fps={effectiveFps()}
          {duration}
          {clipLeft}
          {clipWidth}
          {hasTrim}
          {thumbnailWidth}
          {timeMode}
          {clientXToTime}
        />

        <TimelineZoomLane
          {store}
          {pixelsPerSecond}
          fps={effectiveFps()}
          {duration}
          {timeMode}
          onCopy={copyRegion}
          onDuplicate={duplicateRegion}
        />

        <TimelineAnnotationLane
          {store}
          {pixelsPerSecond}
          fps={effectiveFps()}
          {duration}
          {timeMode}
          onDuplicate={duplicateAnnotation}
        />

        {#if experimentalStore.silenceDetection}
          <TimelineCutLane {store} {pixelsPerSecond} {duration} />
        {/if}
      </div>

      <TimelinePlayhead
        currentTime={store.currentTime}
        fps={effectiveFps()}
        {pixelsPerSecond}
        isDragging={isDraggingPlayhead}
        {timeMode}
        tall={experimentalStore.silenceDetection}
      />
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
