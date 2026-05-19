<script lang="ts">
  import InspectorHint from "$components/editor/InspectorHint.svelte";
  import type { EditorStore } from "$lib/stores/editor-store.svelte";
  import {
    Gauge,
    Maximize2,
    Scissors,
    Search,
    Target,
    VolumeX,
    Wand2,
    ZoomIn,
    ZoomOut
  } from "@lucide/svelte";
  import * as DropdownMenu from "@recast/ui/dropdown-menu";
  import { Kbd } from "@recast/ui/kbd";
  import { cn } from "@recast/ui/utils";
  import SilenceReviewPopover from "../../SilenceReviewPopover.svelte";
  import ZoomSuggestionsPopover from "../../ZoomSuggestionsPopover.svelte";
  import { formatTimeByMode, type TimeMode } from "./timeline-helpers";

  // Top control bar: trim setters, focus/suggest, speed, timeline-zoom and
  // stat chips. Stateless beyond `showSuggestions` and `playbackSpeed` which
  // are owned by the parent so the keyboard handler can read/write them.

  interface Props {
    store: EditorStore;
    fps: number;
    hasTrim: boolean;
    aspectRatioLabel: string;
    frameCount: number;
    showSuggestions: boolean;
    playbackSpeed: number;
    speeds: readonly number[];
    timeMode: TimeMode;
    hasSelectedRegion: boolean;
    onSetTrim: (kind: "in" | "out") => void;
    onAddFocusRegion: () => void;
    onToggleSuggestions: () => void;
    onCloseSuggestions: () => void;
    onResetTrim: () => void;
    onZoomTimeline: (dir: number) => void;
    onSelectSpeed: (speed: number) => void;
    onCycleTimeMode: () => void;
    onZoomToFit: () => void;
    onZoomToSelection: () => void;
  }

  let {
    store,
    fps,
    hasTrim,
    aspectRatioLabel,
    frameCount,
    showSuggestions,
    playbackSpeed,
    speeds,
    timeMode,
    hasSelectedRegion,
    onSetTrim,
    onAddFocusRegion,
    onToggleSuggestions,
    onCloseSuggestions,
    onResetTrim,
    onZoomTimeline,
    onSelectSpeed,
    onCycleTimeMode,
    onZoomToFit,
    onZoomToSelection,
  }: Props = $props();
  let trimHint = `Set trim points to exclude parts of the clip from export, or add focus regions to highlight important moments. You can also ask Trace to suggest focus regions based on where you moved the cursor.`;

  // Silence-review popover state. Kept local — unlike the focus-suggest
  // popover it needs no keyboard integration, so the parent stays unaware.
  let showSilence = $state(false);
</script>

<div
  class="mb-2 flex flex-wrap items-center justify-between gap-2 text-[11px]"
>
  <div class="flex items-center gap-1">
  <InspectorHint content={trimHint}/>
 

    <!-- Trim segmented pill -->
    <div
      class="flex items-center gap-0.5 rounded-lg bg-muted/60 p-0.5 ring-1 ring-inset ring-border/40"
    >
      <button
        type="button"
        onclick={() => onSetTrim("in")}
        title="Cut everything before the playhead (I)"
        class="flex h-6 items-center gap-1 rounded-md px-2 text-[11px] font-semibold text-muted-foreground transition-colors duration-150 hover:bg-card hover:text-foreground"
      >
        <span class="hidden sm:inline">Start here</span>
        <span class="sm:hidden">Start</span>
        <Kbd class="ml-0.5">I</Kbd>
      </button>
      <button
        type="button"
        onclick={() => onSetTrim("out")}
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
        onclick={onAddFocusRegion}
        class="flex h-6 items-center gap-1 rounded-md px-2 text-[11px] font-semibold text-muted-foreground transition-colors duration-150 hover:bg-card hover:text-foreground"
      >
        <Search class="size-3" />
        Focus
      </button>
      <div class="relative">
        <button
          type="button"
          aria-pressed={showSuggestions}
          onclick={onToggleSuggestions}
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
            <ZoomSuggestionsPopover {store} onclose={onCloseSuggestions} />
          </div>
        {/if}
      </div>
    </div>

    <!-- Remove-silence: scans audio + screen motion for dead air -->
    <div class="relative">
      <button
        type="button"
        aria-pressed={showSilence}
        onclick={() => (showSilence = !showSilence)}
        disabled={!store.recordingPath}
        title={store.recordingPath
          ? "Find and remove silent gaps in this recording"
          : "No recording media to analyse"}
        class={cn(
          "flex h-6 items-center gap-1 rounded-md border border-border/40 px-2 text-[11px] font-semibold transition-colors duration-150 disabled:opacity-40",
          showSilence
            ? "bg-card text-foreground shadow-(--shadow-craft-inset) ring-1 ring-inset ring-border/40"
            : "bg-muted/40 text-muted-foreground hover:bg-card hover:text-foreground",
        )}
      >
        <VolumeX class="size-3" />
        Remove silence
        {#if store.cuts.length > 0}
          <span class="rounded bg-primary/15 px-1 text-[9px] font-bold text-primary">
            {store.cuts.length}
          </span>
        {/if}
      </button>
      {#if showSilence}
        <div class="absolute left-0 bottom-full z-40 mb-1.5">
          <SilenceReviewPopover {store} onclose={() => (showSilence = false)} />
        </div>
      {/if}
    </div>

    {#if hasTrim}
      <button
        type="button"
        onclick={onResetTrim}
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
        {#each speeds as speed (speed)}
          <DropdownMenu.Item
            onclick={() => onSelectSpeed(speed)}
            class={playbackSpeed === speed ? "text-primary" : ""}
          >
            {speed.toFixed(2).replace(/\.?0+$/, "")}×
          </DropdownMenu.Item>
        {/each}
      </DropdownMenu.Content>
    </DropdownMenu.Root>

    <!-- Zoom segmented pill: out / level / in / fit-to-clip /
         fit-to-selection. The trailing two buttons are NLE staples and
         resolve the most common scrubbing complaint ("I lost my place"). -->
    <div
      class="flex items-center gap-0.5 rounded-lg bg-muted/60 p-0.5 ring-1 ring-inset ring-border/40"
    >
      <button
        type="button"
        onclick={() => onZoomTimeline(-1)}
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
        onclick={() => onZoomTimeline(1)}
        aria-label="Zoom in timeline"
        class="flex size-6 items-center justify-center rounded-md text-muted-foreground transition-colors duration-150 hover:bg-card hover:text-foreground"
      >
        <ZoomIn class="size-3" />
      </button>
      <button
        type="button"
        onclick={onZoomToFit}
        aria-label="Zoom to fit"
        title="Fit the entire clip in view"
        class="flex size-6 items-center justify-center rounded-md text-muted-foreground transition-colors duration-150 hover:bg-card hover:text-foreground"
      >
        <Maximize2 class="size-3" />
      </button>
      <button
        type="button"
        onclick={onZoomToSelection}
        disabled={!hasSelectedRegion}
        aria-label="Zoom to selection"
        title={hasSelectedRegion
          ? "Zoom in on the selected focus region"
          : "Select a focus region first"}
        class="flex size-6 items-center justify-center rounded-md text-muted-foreground transition-colors duration-150 hover:bg-card hover:text-foreground disabled:opacity-40"
      >
        <Target class="size-3" />
      </button>
    </div>

    <!-- Time-mode pill: cycles smpte → seconds → frames. Persistent state
         lives in the parent so the format applies everywhere at once. -->
    <button
      type="button"
      onclick={onCycleTimeMode}
      aria-label="Cycle time display mode"
      title="Click to cycle: timecode / seconds / frames"
      class="flex h-6 items-center gap-1 rounded-md border border-border/40 bg-muted/40 px-2 font-mono text-[10px] font-semibold uppercase tracking-wide text-muted-foreground transition-colors duration-150 hover:bg-card hover:text-foreground"
    >
      {timeMode === "smpte" ? "TC" : timeMode === "seconds" ? "SEC" : "FR"}
    </button>

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
          {formatTimeByMode(store.clipDuration, timeMode, fps)}
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
