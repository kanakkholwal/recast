<script lang="ts">
  import type { Easing } from "$lib/easing/cubic-bezier";
  import type { EditorStore, ZoomRegion } from "$lib/stores/editor-store.svelte";
  import { CircleQuestionMark, Scissors, Search, Wand2, X, Gauge, ZoomIn, ZoomOut } from "@lucide/svelte";
  import { Badge } from "@recast/ui/badge";
  import { Button } from "@recast/ui/button";
  import * as DropdownMenu from "@recast/ui/dropdown-menu";
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
  let showTrimHelp = $state(false);
  let showSuggestions = $state(false);
  let timelineWidth = $state(900);
  let activeTrimHandle = $state<"in" | "out" | null>(null);

  const MIN_TRIM_GAP = 0.1; // seconds

  const SPEEDS = [0.25, 0.5, 1.0, 1.5, 2.0];
  let playbackSpeed = $state(1.0);

  $effect(() => {
    if (videoEl) videoEl.playbackRate = playbackSpeed;
  });

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
    store.pushUndoState();
    activeTrimHandle = which;
    (event.currentTarget as Element).setPointerCapture(event.pointerId);
    updateTrimFromPointer(event.clientX, which);
    const onMove = (e: PointerEvent) => {
      updateTrimFromPointer(e.clientX, which);
    };
    const onUp = (e: PointerEvent) => {
      activeTrimHandle = null;
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

  function updateTrimFromPointer(clientX: number, which: "in" | "out") {
    const t = clientXToTime(clientX);
    const effectiveEnd = store.trimEnd || duration;
    if (which === "in") {
      const clamped = Math.min(t, Math.max(0, effectiveEnd - MIN_TRIM_GAP));
      store.trimStart = Math.max(0, clamped);
    } else {
      const clamped = Math.max(t, Math.min(duration, store.trimStart + MIN_TRIM_GAP));
      store.trimEnd = Math.min(duration, clamped);
    }
  }

  function handleTrimHandleKey(event: KeyboardEvent, which: "in" | "out") {
    if (duration <= 0) return;
    const step = event.shiftKey
      ? 1
      : store.metadata?.fps
        ? 1 / store.metadata.fps
        : 1 / 30;
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    event.stopPropagation();
    const delta = event.key === "ArrowLeft" ? -step : step;
    if (which === "in") {
      const effectiveEnd = store.trimEnd || duration;
      const next = Math.max(0, Math.min(effectiveEnd - MIN_TRIM_GAP, store.trimStart + delta));
      store.trimStart = next;
    } else {
      const next = Math.max(
        store.trimStart + MIN_TRIM_GAP,
        Math.min(duration, (store.trimEnd || duration) + delta),
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
  const clipLeft = $derived(store.trimStart * pixelsPerSecond);
  const clipRight = $derived((store.trimEnd || duration) * pixelsPerSecond);
  const clipWidth = $derived(Math.max(clipRight - clipLeft, 0));
  const thumbnailWidth = $derived(
    store.thumbnailStrip.length > 0
      ? Math.max(88, clipWidth / store.thumbnailStrip.length)
      : 112,
  );
  const hasTrim = $derived(
    store.trimStart > 0 ||
      ((store.metadata?.duration ?? 0) > 0 &&
        store.trimEnd > 0 &&
        store.trimEnd < (store.metadata?.duration ?? 0)),
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

    const step = event.shiftKey
      ? 1
      : store.metadata?.fps
        ? 1 / store.metadata.fps
        : 1 / 30;

    if (event.key === "ArrowLeft") {
      event.preventDefault();
      const next = Math.max(0, store.currentTime - step);
      store.currentTime = next;
      if (videoEl) videoEl.currentTime = next;
    }

    if (event.key === "ArrowRight") {
      event.preventDefault();
      const next = Math.min(duration, store.currentTime + step);
      store.currentTime = next;
      if (videoEl) videoEl.currentTime = next;
    }

    // Final Cut-style in/out point shortcuts.
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
    const clipEnd = store.trimEnd || duration;
    const start = Math.max(store.trimStart, store.currentTime - 0.35);
    const end = Math.min(
      clipEnd,
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
    const minGap = 0.1;
    if (kind === "in") {
      const nextIn = Math.min(
        store.currentTime,
        Math.max(0, (store.trimEnd || duration) - minGap),
      );
      store.trimStart = nextIn;
      if (store.currentTime < nextIn) {
        store.currentTime = nextIn;
      }
    } else {
      const nextOut = Math.max(
        store.currentTime,
        Math.min(duration, store.trimStart + minGap),
      );
      store.trimEnd = nextOut;
      if (store.currentTime > nextOut) {
        store.currentTime = nextOut;
      }
    }
    syncVideoTime();
  }

  function resetTrim() {
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
  class="shrink-0 select-none border-t border-border bg-card/30 px-3 pb-2 pt-2"
>
  <div
    class="mb-2 flex flex-wrap items-center justify-between gap-2 text-[11px]"
  >
    <div class="flex items-center gap-1">
      <HoverCard>
        <HoverCardTrigger type="button">
          <CircleQuestionMark class="size-4" />
        </HoverCardTrigger>
        <HoverCardContent alignOffset={20}>
          Set <span class="text-foreground">In</span> and
          <span class="text-foreground">Out</span> at the playhead to shorten the
          clip. The shaded segment is what will render and export.
        </HoverCardContent>
      </HoverCard>
      <Button
        type="button"
        size="xs"
        variant="outline"
        onclick={() => setTrimPoint("in")}
      >
        Set In
      </Button>
      <Button
        type="button"
        size="xs"
        variant="outline"
        onclick={() => setTrimPoint("out")}
      >
        Set Out
      </Button>
      <Button
        type="button"
        size="xs"
        variant="outline"
        onclick={addFocusRegion}
      >
        <Search size={11} />
        Focus
      </Button>
      <div class="relative">
        <Button
          type="button"
          size="xs"
          variant="outline"
          aria-pressed={showSuggestions}
          onclick={toggleSuggestions}
          disabled={!store.cursorPath}
          title={store.cursorPath ? "Suggest focus regions from captured cursor activity" : "No cursor data in this clip"}
        >
          <Wand2 size={11} />
          Suggest
        </Button>
        {#if showSuggestions}
          <div class="absolute left-0 bottom-full z-40 mt-1.5">
            <ZoomSuggestionsPopover {store} onclose={() => (showSuggestions = false)} />
          </div>
        {/if}
      </div>
      {#if hasTrim}
        <Button type="button" size="xs" variant="outline" onclick={resetTrim}>
          <Scissors size={11} />
          Reset Trim
        </Button>
      {/if}
    </div>

    <div class="flex items-center gap-2 text-muted-foreground">
      <!-- Speed menu -->
      <DropdownMenu.Root>
        <DropdownMenu.Trigger>
          <Button
            variant="ghost"
            size="xs"
            class="gap-1 font-mono tabular-nums h-6 text-muted-foreground hover:text-foreground"
            aria-label="Playback speed"
          >
            <Gauge size={12} />
            {playbackSpeed.toFixed(2).replace(/\.?0+$/, "")}×
          </Button>
        </DropdownMenu.Trigger>
        <DropdownMenu.Content align="end">
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

      <div class="mx-1 h-4 w-px bg-border"></div>

      <div class="flex items-center gap-0.5">
        <Button
          variant="ghost"
          size="icon-sm"
          onclick={() => zoomTimeline(-1)}
          aria-label="Zoom out timeline"
          class="size-6 text-muted-foreground hover:text-foreground"
        >
          <ZoomOut size={13} />
        </Button>

        <span class="min-w-8 text-center font-mono tabular-nums text-[10px] font-medium text-foreground">
          {store.timelineZoom.toFixed(1)}x
        </span>

        <Button
          variant="ghost"
          size="icon-sm"
          onclick={() => zoomTimeline(1)}
          aria-label="Zoom in timeline"
          class="size-6 text-muted-foreground hover:text-foreground"
        >
          <ZoomIn size={13} />
        </Button>
      </div>

      <div class="mx-1 h-4 w-px bg-border"></div>

      <Badge variant="secondary" class="font-mono text-[10px]">
        {aspectRatioLabel}
      </Badge>
      <Badge variant="secondary" class="font-mono text-[10px]">
        {frameCount} frames
      </Badge>

      <span class="inline-flex items-center gap-1 ml-1">
        <kbd
          class="rounded border border-border bg-background px-1 py-0.5 font-mono text-[10px]"
          >Scroll</kbd
        >
        <span>pan</span>
      </span>
      <span class="inline-flex items-center gap-1">
        <kbd
          class="rounded border border-border bg-background px-1 py-0.5 font-mono text-[10px]"
          >⌘ Scroll</kbd
        >
        <span>zoom</span>
      </span>
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
    class="custom-scrollbar relative overflow-x-auto overflow-y-hidden rounded-lg border border-border bg-background/60"
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
      <div class="relative h-7 border-b border-border bg-muted/20">
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
          class="relative h-12 rounded-md border border-border bg-background"
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
              {hasTrim ? "Trimmed" : "Full clip"}
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
              aria-label="Trim in"
              aria-valuemin={0}
              aria-valuemax={duration}
              aria-valuenow={store.trimStart}
              aria-valuetext={formatTime(store.trimStart)}
              onpointerdown={(e) => startTrimDrag(e, "in")}
              onkeydown={(e) => handleTrimHandleKey(e, "in")}
              class="group absolute inset-y-0 left-0 z-10 w-2 -translate-x-1 cursor-ew-resize focus-visible:outline-none"
            >
              <div class="mx-auto h-full w-1 rounded-l-md bg-primary transition-all group-hover:w-1.5 group-hover:shadow-[0_0_0_2px_rgba(59,130,246,0.3)]"></div>
              {#if activeTrimHandle === "in"}
                <div
                  class="pointer-events-none absolute bottom-full left-1/2 mb-1 -translate-x-1/2 whitespace-nowrap rounded border border-border bg-popover px-1.5 py-0.5 font-mono text-[9px] text-foreground shadow-sm"
                >
                  {formatTime(store.trimStart)}
                </div>
              {/if}
            </div>
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              role="slider"
              tabindex="0"
              aria-label="Trim out"
              aria-valuemin={0}
              aria-valuemax={duration}
              aria-valuenow={store.trimEnd || duration}
              aria-valuetext={formatTime(store.trimEnd || duration)}
              onpointerdown={(e) => startTrimDrag(e, "out")}
              onkeydown={(e) => handleTrimHandleKey(e, "out")}
              class="group absolute inset-y-0 right-0 z-10 w-2 translate-x-1 cursor-ew-resize focus-visible:outline-none"
            >
              <div class="mx-auto h-full w-1 rounded-r-md bg-primary transition-all group-hover:w-1.5 group-hover:shadow-[0_0_0_2px_rgba(59,130,246,0.3)]"></div>
              {#if activeTrimHandle === "out"}
                <div
                  class="pointer-events-none absolute bottom-full left-1/2 mb-1 -translate-x-1/2 whitespace-nowrap rounded border border-border bg-popover px-1.5 py-0.5 font-mono text-[9px] text-foreground shadow-sm"
                >
                  {formatTime(store.trimEnd || duration)}
                </div>
              {/if}
            </div>
          </div>
        </div>

        <div
          class="mt-1.5 min-h-9 rounded-md border border-border bg-background/40 px-1.5 py-1.5"
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
                onclick={() => (store.selectedZoomRegionId = region.id)}
                in:fly={{ y: 10, duration: 180, easing: cubicOut }}
                out:fade={{ duration: 140 }}
                aria-pressed={isSelected}
                class="absolute overflow-hidden rounded border bg-muted text-left transition-colors focus:outline-none focus:ring-1 focus:ring-ring {isSelected ? 'border-primary ring-1 ring-primary/40' : 'border-border hover:border-primary/60'}"
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
                    tabindex="0"
                    onclick={(event) => {
                      event.stopPropagation();
                      store.removeZoomRegion(region.id);
                    }}
                    onkeydown={(event) => {
                      if (event.key === "Enter" || event.key === " ") {
                        event.preventDefault();
                        event.stopPropagation();
                        store.removeZoomRegion(region.id);
                      }
                    }}
                    class="flex h-4 w-4 shrink-0 cursor-pointer items-center justify-center rounded border border-border bg-background/70 text-muted-foreground transition-colors hover:border-destructive hover:text-destructive"
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
            {formatTime(store.currentTime)}
          </div>
          <div
            class="mx-auto mt-6 size-2 rounded-full border border-background bg-primary"
          ></div>
          <div class="mx-auto h-31.5 w-px bg-primary/60"></div>
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
