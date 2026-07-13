<script lang="ts">
  import type { EditorStore } from "$lib/stores/editor-store.svelte";
  import { originalToOutput, outputToOriginal } from "$lib/timeline/time-map";
  import {
    type FilmstripTile,
    planFilmstrip,
  } from "$lib/timeline/filmstrip";
  import type { TileProvider } from "$lib/timeline/filmstrip-source";
  import { deriveSeams } from "$lib/timeline/segments";
  import { Gauge, RotateCcw, SquareSplitHorizontal, Trash2 } from "@lucide/svelte";
  import * as ContextMenu from "@recast/ui/context-menu";
  import { fade } from "svelte/transition";
  import {
    buildWaveformPath,
    formatTimeByMode,
    formatSmpte,
    frameStep,
    minClipDuration,
    type TimeMode,
  } from "./timeline-helpers";
  import {
    clampTrimIn,
    clampTrimOut,
    layoutClipBlocks,
    nudgeTrimIn,
    nudgeTrimOut,
  } from "./timeline-clipbar.logic";
  import { buildSnapTargets, snapTime } from "./timeline-snap";

  // Clip bar with thumbnails and in/out trim handles. Owns its drag state;
  // the parent only supplies `clientXToOutput` (handles scroll offset) to resolve pointer X.

  interface Props {
    store: EditorStore;
    videoEl: HTMLVideoElement | null;
    fps: number;
    duration: number;
    pixelsPerSecond: number;
    clipLeft: number;
    clipWidth: number;
    thumbnailWidth: number;
    timeMode: TimeMode;
    /** What fills the clip bar: thumbnails or the audio waveform, never both. */
    content: "thumbnails" | "waveform";
    /** Pointer clientX → output seconds (pre-map); trim maps it via a frozen map. */
    clientXToOutput: (clientX: number) => number;
    // Density-based filmstrip. When null, the stretched Rust strip is rendered.
    tileProvider: TileProvider | null;
    filmstripVersion: number;
    viewportLeftPx: number;
    viewportWidthPx: number;
  }

  let {
    store,
    videoEl,
    fps,
    duration,
    pixelsPerSecond,
    clipLeft,
    clipWidth,
    thumbnailWidth,
    timeMode,
    content,
    clientXToOutput,
    tileProvider,
    filmstripVersion,
    viewportLeftPx,
    viewportWidthPx,
  }: Props = $props();

  // Target tile width and the cache-key height namespace; overscan decodes a bit
  // beyond the viewport so tiles are ready just before they scroll in.
  const TILE_TARGET_W = 96;
  const TILE_KEY_HEIGHT = 48;
  const FILMSTRIP_OVERSCAN = 240;

  const formatSpeed = (s: number) => `${s}×`;

  // One block per kept segment on the OUTPUT (post-cut) axis: a cut occupies zero
  // width and later clips slide left to close the gap. `xOf` maps original time onto that axis.
  const pps = $derived(pixelsPerSecond);
  const xOf = (t: number) => originalToOutput(store.timeMap, t) * pps;
  // Thumbnail strip is laid across this; each block is internally cut-free, so it shows its slice via a margin offset.
  const clipDuration = $derived(Math.max(0.0001, store.outPoint - store.inPoint));
  const stripFullWidth = $derived(clipDuration * pps);
  const thumbW = $derived(
    store.thumbnailStrip.length > 0
      ? Math.max(2, stripFullWidth / store.thumbnailStrip.length)
      : thumbnailWidth,
  );
  const clipBlocks = $derived(
    layoutClipBlocks(store.segments, xOf, pps, store.inPoint),
  );
  // Density-based filmstrip tiles, planned per kept block and virtualized to the
  // viewport. Empty (fallback to the stretched strip) when there's no provider.
  const filmstripTiles = $derived(
    tileProvider
      ? planFilmstrip(
          clipBlocks.map((b) => ({
            key: b.key,
            leftPx: b.left,
            widthPx: b.width,
            originalStart: b.start,
            originalEnd: b.end,
          })),
          { leftPx: viewportLeftPx, widthPx: viewportWidthPx },
          {
            tileWidthPx: TILE_TARGET_W,
            tileHeightPx: TILE_KEY_HEIGHT,
            overscanPx: FILMSTRIP_OVERSCAN,
          },
        )
      : [],
  );
  const tilesByBlock = $derived.by(() => {
    const map = new Map<number, FilmstripTile[]>();
    for (const tile of filmstripTiles) {
      const list = map.get(tile.blockKey);
      if (list) list.push(tile);
      else map.set(tile.blockKey, [tile]);
    }
    return map;
  });
  // Tile URLs, re-resolved whenever a freshly decoded tile bumps the version.
  const tileUrls = $derived.by(() => {
    void filmstripVersion;
    const map = new Map<string, string | undefined>();
    if (tileProvider) {
      for (const tile of filmstripTiles) {
        map.set(tile.cacheKey, tileProvider.get(tile));
      }
    }
    return map;
  });
  $effect(() => {
    if (tileProvider && filmstripTiles.length > 0) {
      tileProvider.request(filmstripTiles);
    }
  });

  const splitMarkers = $derived(
    store.splitPoints
      .filter((p) => p > store.inPoint && p < store.outPoint)
      .map((p) => ({ time: p, x: xOf(p) })),
  );
  // Where a removed cut sits between two kept segments, collapsed to one seam.
  // deriveSeams is the pure unit-tested helper; here we only add the output-axis x.
  const seamMarkers = $derived(
    deriveSeams(store.segments).map((s) => ({ ...s, x: xOf(s.gapStart) })),
  );
  function restoreSeam(gapStart: number, gapEnd: number) {
    // Restore every cut that lives inside the collapsed gap.
    for (const c of store.effectiveCuts) {
      if (c.start >= gapStart - 1e-3 && c.end <= gapEnd + 1e-3) {
        store.removeCut(c.id);
      }
    }
  }
  const inHandleLeft = $derived(clipLeft);
  const outHandleLeft = $derived(clipLeft + clipWidth);

  // Midpoint is always inside the block, so deleteSegmentAt targets exactly it; park playhead on the join.
  function deleteSegment(start: number, end: number) {
    const joinAt = store.deleteSegmentAt((start + end) / 2);
    if (joinAt === null) return;
    store.currentTime = joinAt;
    if (videoEl) videoEl.currentTime = joinAt;
  }

  // Right-click menu: original time the menu was opened at (set on pointerdown,
  // which fires for the right button before `contextmenu`), so "Split here"
  // splits exactly where you clicked rather than at the playhead.
  const SPEED_PRESETS = [0.5, 1, 1.5, 2] as const;
  let menuTime = $state(0);
  function rememberMenuTime(clientX: number) {
    menuTime = outputToOriginal(store.timeMap, clientXToOutput(clientX));
  }

  // Faint audio envelope over the footage, so you can see where to cut. Built in
  // output-pixel space (each bucket at `xOf(bucketTime)`) over the kept range
  // only; buckets inside a removed cut collapse onto the seam like the cut lane.
  const WAVE_H = 48;
  const waveformPath = $derived(
    buildWaveformPath({
      waveform: store.waveform,
      duration,
      xOf,
      height: WAVE_H,
      amp: WAVE_H / 2 - 3,
      range: { start: store.inPoint, end: store.outPoint },
    }),
  );

  let activeTrimHandle = $state<"in" | "out" | null>(null);
  // Output-x of the active trim snap target (playhead/region/etc.), or null.
  let trimSnapX = $state<number | null>(null);

  // `originalAt` = the handle value at pointer-down, for the frames-delta tooltip.
  let trimDragContext = $state<{
    which: "in" | "out";
    originalAt: number;
  } | null>(null);
  function startTrimDrag(event: PointerEvent, which: "in" | "out") {
    if (duration <= 0) return;
    event.preventDefault();
    event.stopPropagation();
    // Single undo entry per drag.
    store.pushUndoState();
    // Un-collapse the axis for the drag: the clip un-brackets to the full
    // recording (trimmed head/tail ghosted), the handle follows the cursor, and
    // dragging outward restores. Reverts to the collapsed view on pointer-up.
    store.isTrimming = true;
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
      trimSnapX = null;
      store.isTrimming = false;
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

  // Snap the dragged handle to the playhead, clip edges, and region/annotation
  // boundaries (not its own point); falls through to the frame grid otherwise.
  function snapTrim(raw: number, which: "in" | "out"): number {
    const targets = buildSnapTargets({
      playhead: store.currentTime,
      inPoint: store.inPoint,
      outPoint: store.outPoint,
      duration,
      regions: store.zoomRegions,
      annotations: store.annotations,
    }).filter((target) =>
      which === "in"
        ? target.kind !== "in-point"
        : target.kind !== "out-point",
    );
    const tolerance = pps > 0 ? 6 / pps : 0;
    const result = snapTime(raw, targets, tolerance, fps);
    // Surface the active snap as a guide line at the target's position.
    trimSnapX = result.target ? xOf(result.target.time) : null;
    return result.time;
  }

  function updateTrimFromPointer(
    clientX: number,
    which: "in" | "out",
    scrub = false,
  ) {
    // Output px → original time. While trimming, store.timeMap is the full
    // recording axis (stable, not collapsing under the drag), so absolute
    // mapping tracks the cursor and lets the handle move across the whole source.
    const raw = outputToOriginal(store.timeMap, clientXToOutput(clientX));
    const t = snapTrim(raw, which);
    const min = minClipDuration(fps);
    if (which === "in") {
      const next = clampTrimIn(t, store.outPoint, min);
      store.trimStart = next;
      // Park playback at the in point so the preview shows the first kept frame while dragging.
      if (scrub) {
        store.currentTime = next;
        if (videoEl) videoEl.currentTime = next;
      }
    } else {
      const next = clampTrimOut(t, duration, store.inPoint, min);
      store.trimEnd = next;
      if (scrub) {
        // Show the last kept frame (one before the cut), the frame being decided on.
        const previewAt = Math.max(store.inPoint, next - frameStep(fps));
        store.currentTime = previewAt;
        if (videoEl) videoEl.currentTime = previewAt;
      }
    }
  }

  function nudgeTrimByKey(which: "in" | "out", direction: 1 | -1, second: boolean) {
    if (duration <= 0) return;
    store.pushUndoStateCoalesced(`trim-${which}`, 500);
    const delta = direction * (second ? 1 : frameStep(fps));
    const min = minClipDuration(fps);
    if (which === "in") {
      store.trimStart = nudgeTrimIn(store.inPoint, store.outPoint, delta, min, fps);
    } else {
      store.trimEnd = nudgeTrimOut(
        store.inPoint,
        store.outPoint,
        duration,
        delta,
        min,
        fps,
      );
    }
  }

  function handleTrimHandleKey(event: KeyboardEvent, which: "in" | "out") {
    if (duration <= 0) return;
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    event.stopPropagation();
    nudgeTrimByKey(which, event.key === "ArrowLeft" ? -1 : 1, event.shiftKey);
  }
</script>

<div class="relative h-12">
  <!-- Ghost bands: while trimming, the axis un-collapses to the full recording
       and the trimmed head/tail show dimmed, so you can see and re-drag them. -->
  {#if store.isTrimming}
    {#if clipLeft > 1}
      <div
        class="pointer-events-none absolute inset-y-0 left-0 z-8 rounded-l-md border border-border/40 bg-background/60"
        style="width: {clipLeft}px;"
      ></div>
    {/if}
    <div
      class="pointer-events-none absolute inset-y-0 right-0 z-8 rounded-r-md border border-border/40 bg-background/60"
      style="left: {clipLeft + clipWidth}px;"
    ></div>
    {#if trimSnapX !== null}
      <div
        class="pointer-events-none absolute inset-y-0 z-9 w-px bg-primary"
        style="left: {trimSnapX}px;"
      ></div>
    {/if}
  {/if}

  {#each clipBlocks as block (block.key)}
    {@const selected =
      store.selectedClipStart !== null &&
      Math.abs(store.selectedClipStart - block.start) < 1e-4}
    {@const speed = store.segmentSpeedAt(block.start)}
    <ContextMenu.Root>
      <ContextMenu.Trigger>
        {#snippet child({ props })}
          <!-- Select on POINTERDOWN, not click: the timeline scroller calls
               setPointerCapture on its own pointerdown, which redirects pointerup
               and makes the synthesised click land on the scroller (seek) instead
               of here. We don't stop propagation, so the click still seeks too
               (select + seek). Right-click records the time for "Split here". -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            {...props}
            role="button"
            tabindex="-1"
            data-selectable
            onpointerdown={(e) => {
              rememberMenuTime(e.clientX);
              if (e.button === 0) store.selectedClipStart = block.start;
            }}
            class="group/clip absolute inset-y-0 cursor-pointer overflow-hidden rounded-md border bg-primary/5 transition-[box-shadow,border-color] {selected
              ? 'border-primary ring-2 ring-primary/50'
              : 'border-primary/40 hover:border-primary/70'}"
            style="left: {block.left}px; width: {block.width}px;"
          >
      {#if content === "thumbnails"}
        {#if tileProvider}
        {#each tilesByBlock.get(block.key) ?? [] as tile (tile.cacheKey)}
          {@const url = tileUrls.get(tile.cacheKey)}
          <div
            class="absolute inset-y-0 overflow-hidden"
            style="left: {tile.offsetPx}px; width: {tile.widthPx}px;"
          >
            {#if url}
              <img
                in:fade={{ duration: 120 }}
                src={url}
                alt=""
                class="h-full w-full object-cover"
                draggable="false"
              />
            {:else}
              <div class="h-full w-full bg-muted/40"></div>
            {/if}
          </div>
        {/each}
      {:else if store.thumbnailStrip.length > 0}
        <div
          class="flex h-full"
          style="width: {stripFullWidth}px; margin-left: {block.stripOffset}px;"
        >
          {#each store.thumbnailStrip as frame, index (frame + index)}
            <img
              in:fade={{ duration: 180 }}
              src={frame}
              alt="Timeline frame"
              class="h-full shrink-0 object-cover"
              style="width: {thumbW}px;"
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
      {/if}

      <!-- Read-only speed badge (the editable control lives in the Clip panel). -->
      {#if speed !== 1}
        <div
          title="Clip speed. Edit in the Clip panel."
          class="pointer-events-none absolute left-1 top-1 z-7 flex h-4 items-center gap-0.5 rounded bg-primary/90 px-1 font-mono text-[9px] font-bold text-primary-foreground"
        >
          <Gauge class="size-2.5" />
          {formatSpeed(speed)}
        </div>
      {/if}

      <!-- Per-clip ripple delete, only with >1 clip (trim handles remove the whole recording). -->
      {#if clipBlocks.length > 1}
        <!-- svelte-ignore a11y_consider_explicit_label -->
        <button
          type="button"
          onpointerdown={(e) => e.stopPropagation()}
          onclick={() => deleteSegment(block.start, block.end)}
          title="Delete this clip and close the gap"
          class="absolute right-1 top-1 z-7 flex size-4 items-center justify-center rounded bg-background/80 text-muted-foreground opacity-0 backdrop-blur transition-opacity hover:bg-destructive hover:text-destructive-foreground group-hover/clip:opacity-100"
        >
          <Trash2 class="size-2.5" />
        </button>
      {/if}
          </div>
        {/snippet}
      </ContextMenu.Trigger>
      <ContextMenu.Content size="sm" class="w-48">
        <ContextMenu.Item onSelect={() => store.splitAt(menuTime)}>
          <SquareSplitHorizontal />
          Split here
        </ContextMenu.Item>
        <ContextMenu.Sub>
          <ContextMenu.SubTrigger>
            <Gauge />
            Speed
          </ContextMenu.SubTrigger>
          <ContextMenu.SubContent>
            <ContextMenu.RadioGroup
              value={String(speed)}
              onValueChange={(v) =>
                store.setSegmentSpeed(block.start, parseFloat(v))}
            >
              {#each SPEED_PRESETS as preset (preset)}
                <ContextMenu.RadioItem value={String(preset)}>
                  {formatSpeed(preset)}
                </ContextMenu.RadioItem>
              {/each}
            </ContextMenu.RadioGroup>
          </ContextMenu.SubContent>
        </ContextMenu.Sub>
        <ContextMenu.Item
          disabled={speed === 1}
          onSelect={() => store.setSegmentSpeed(block.start, 1)}
        >
          <RotateCcw />
          Reset speed
        </ContextMenu.Item>
        {#if clipBlocks.length > 1}
          <ContextMenu.Separator />
          <ContextMenu.Item
            variant="destructive"
            onSelect={() => deleteSegment(block.start, block.end)}
          >
            <Trash2 />
            Delete clip
          </ContextMenu.Item>
        {/if}
      </ContextMenu.Content>
    </ContextMenu.Root>
  {/each}

  <!-- Audio waveform fills the clip bar when it's the chosen content (the radio
       in the Layers menu), so it never overlaps the thumbnails. -->
  {#if content === "waveform" && waveformPath && clipWidth > 0}
    <svg
      class="pointer-events-none absolute inset-y-0 left-0"
      style="width: {clipWidth}px;"
      viewBox="0 0 {clipWidth} {WAVE_H}"
      preserveAspectRatio="none"
      aria-hidden="true"
    >
      <path d={waveformPath} class="fill-primary/45" />
    </svg>
  {/if}

  <!-- Removed section collapsed to a restorable seam (click to restore). -->
  {#each seamMarkers as seam (seam.gapStart)}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <button
      type="button"
      onpointerdown={(e) => e.stopPropagation()}
      onclick={() => restoreSeam(seam.gapStart, seam.gapEnd)}
      title="Removed {seam.removed.toFixed(2)}s. Click to restore."
      class="group/seam absolute inset-y-0 z-6 w-3 -translate-x-1/2 cursor-pointer"
      style="left: {seam.x}px;"
    >
      <!-- Notched destructive seam: two small triangles meeting at a hairline,
           reading as "content was pinched out here". -->
      <div
        class="mx-auto h-full w-0.5 bg-destructive/70 transition-all group-hover/seam:w-1 group-hover/seam:bg-destructive"
      ></div>
      <span
        class="pointer-events-none absolute bottom-full left-1/2 mb-1 hidden -translate-x-1/2 whitespace-nowrap rounded border border-border bg-popover px-1.5 py-0.5 font-mono text-[9px] text-foreground shadow-sm group-hover/seam:block"
      >
        −{seam.removed.toFixed(2)}s · restore
      </span>
    </button>
  {/each}

  <!-- Split seams between adjacent clips. Double-click to rejoin. -->
  {#each splitMarkers as marker (marker.time)}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      role="presentation"
      onpointerdown={(e) => e.stopPropagation()}
      ondblclick={() => store.removeSplit(marker.time)}
      title="Split. Double-click to rejoin."
      class="group/split absolute inset-y-0 z-6 w-2 -translate-x-1/2 cursor-pointer"
      style="left: {marker.x}px;"
    >
      <div
        class="mx-auto h-full w-px bg-warning transition-all group-hover/split:w-0.5"
      ></div>
    </div>
  {/each}

  <!--
    Trim drag handles, anchored to the in/out points. Each is a narrow vertical
    bar with a larger invisible hit area so grabbing is easy. Pointer events
    stop propagation so we don't fight the timeline's click-to-seek scrub.
  -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    role="slider"
    tabindex="0"
    aria-label="In point"
    aria-valuemin={0}
    aria-valuemax={duration}
    aria-valuenow={store.inPoint}
    aria-valuetext={formatSmpte(store.inPoint, fps)}
    onpointerdown={(e) => startTrimDrag(e, "in")}
    onkeydown={(e) => handleTrimHandleKey(e, "in")}
    class="group absolute inset-y-0 z-10 w-2 -translate-x-1 cursor-ew-resize focus-visible:outline-none"
    style="left: {inHandleLeft}px;"
  >
    <div
      class="mx-auto h-full w-1 rounded-l-md bg-primary transition-all group-hover:w-1.5 group-hover:ring-2 group-hover:ring-primary/30"
    ></div>
    {#if activeTrimHandle === "in" && trimDragContext}
      {@const delta = store.inPoint - trimDragContext.originalAt}
      <div
        class="pointer-events-none absolute bottom-full left-1/2 mb-1 flex -translate-x-1/2 items-center gap-1.5 whitespace-nowrap rounded border border-border bg-popover px-1.5 py-0.5 font-mono text-[9px] text-foreground shadow-sm"
      >
        <span>In {formatTimeByMode(store.inPoint, timeMode, fps)}</span>
        {#if delta !== 0}
          <span class="text-muted-foreground"
            >{delta > 0 ? "+" : ""}{Math.round(delta * fps)} f</span
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
    aria-valuetext={formatSmpte(store.outPoint, fps)}
    onpointerdown={(e) => startTrimDrag(e, "out")}
    onkeydown={(e) => handleTrimHandleKey(e, "out")}
    class="group absolute inset-y-0 z-10 w-2 -translate-x-1 cursor-ew-resize focus-visible:outline-none"
    style="left: {outHandleLeft}px;"
  >
    <div
      class="mx-auto h-full w-1 rounded-r-md bg-primary transition-all group-hover:w-1.5 group-hover:ring-2 group-hover:ring-primary/30"
    ></div>
    {#if activeTrimHandle === "out" && trimDragContext}
      {@const delta = store.outPoint - trimDragContext.originalAt}
      <div
        class="pointer-events-none absolute bottom-full left-1/2 mb-1 flex -translate-x-1/2 items-center gap-1.5 whitespace-nowrap rounded border border-border bg-popover px-1.5 py-0.5 font-mono text-[9px] text-foreground shadow-sm"
      >
        <span>Out {formatTimeByMode(store.outPoint, timeMode, fps)}</span>
        {#if delta !== 0}
          <span class="text-muted-foreground"
            >{delta > 0 ? "+" : ""}{Math.round(delta * fps)} f</span
          >
        {/if}
      </div>
    {/if}
  </div>
</div>
