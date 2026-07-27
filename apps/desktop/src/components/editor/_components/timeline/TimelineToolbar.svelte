<script lang="ts">
  import InspectorHint from "$components/editor/InspectorHint.svelte";
  import { experimentalStore } from "$lib/stores/experimental.svelte";
  import type { EditorStore } from "$lib/stores/editor-store.svelte";
  import { AudioLines, Clapperboard, Clock, Expand, Eye, FastForward, Keyboard, Layers, Maximize2, Minus, Pencil, Plus, Redo2, Scissors, SlidersHorizontal, SquareSplitHorizontal, Target, Undo2, VolumeX, ZoomIn, AiWand } from "@recast/icons";
  import * as DropdownMenu from "@recast/ui/dropdown-menu";
  import { Kbd } from "@recast/ui/kbd";
  import * as Popover from "@recast/ui/popover";
  import { cn } from "@recast/ui/utils";
  import SilenceReviewPopover from "../../SilenceReviewPopover.svelte";
  import ZoomSuggestionsPopover from "../../ZoomSuggestionsPopover.svelte";
  import { formatTimeByMode, type TimeMode } from "./timeline-helpers";

  // Three clusters: EDIT (split + trim to playhead) · INSERT (focus/suggest/
  // silence) · VIEW (zoom + display options). Popovers are portalled because the
  // timeline lives in an `overflow-hidden` slide wrapper that would clip them.

  interface Props {
    store: EditorStore;
    fps: number;
    hasTrim: boolean;
    aspectRatioLabel: string;
    frameCount: number;
    playbackSpeed: number;
    speeds: readonly number[];
    timeMode: TimeMode;
    hasSelectedRegion: boolean;
    razorActive: boolean;
    showAudioLane: boolean;
    showZoomLane: boolean;
    showMarkupLane: boolean;
    showCutLane: boolean;
    showCutGaps: boolean;
    onSetTrim: (kind: "in" | "out") => void;
    onSplit: () => void;
    onToggleRazor: () => void;
    onToggleAudioLane: () => void;
    onToggleZoomLane: () => void;
    onToggleMarkupLane: () => void;
    onToggleCutLane: () => void;
    onToggleCutGaps: () => void;
    onAddFocusRegion: () => void;
    onResetTrim: () => void;
    onZoomTimeline: (dir: number) => void;
    onSelectSpeed: (speed: number) => void;
    onSetTimeMode: (mode: TimeMode) => void;
    onZoomToFit: () => void;
    onZoomToSelection: () => void;
  }

  let {
    store,
    fps,
    hasTrim,
    aspectRatioLabel,
    frameCount,
    playbackSpeed,
    speeds,
    timeMode,
    hasSelectedRegion,
    razorActive,
    showAudioLane,
    showZoomLane,
    showMarkupLane,
    showCutLane,
    showCutGaps,
    onSetTrim,
    onSplit,
    onToggleRazor,
    onToggleAudioLane,
    onToggleZoomLane,
    onToggleMarkupLane,
    onToggleCutLane,
    onToggleCutGaps,
    onAddFocusRegion,
    onResetTrim,
    onZoomTimeline,
    onSelectSpeed,
    onSetTimeMode,
    onZoomToFit,
    onZoomToSelection,
  }: Props = $props();

  const trimHint = `Set in/out points (I/O) to keep the ends you want. Remove a section with the Cut tool (C, click two points), by splitting at the playhead (S) and deleting the clip, or by dragging across the Cuts lane. Add zoom regions to highlight moments; Recast can also suggest them from your cursor activity.`;

  let suggestOpen = $state(false);
  let showSilence = $state(false);

  // Counts only silence-detected cuts; manual ripple deletes shouldn't inflate this.
  const silenceCutCount = $derived(
    store.cuts.filter((c) => c.source === "silence").length,
  );

  // How many export-affecting effects are currently switched off. Surfaced as a
  // badge on the Layers button so "my cuts didn't apply" is visible without
  // opening the menu: this is the state that changes the output file, unlike the
  // lane-visibility toggles above it (which are purely cosmetic).
  const effectsOff = $derived(
    (store.cutsEnabled ? 0 : 1) +
      (store.focusEnabled ? 0 : 1) +
      (store.annotationsGloballyHidden ? 1 : 0),
  );

  // Shared control styling so every toolbar affordance reads the same.
  const GROUP =
    "flex items-center gap-0.5 rounded-lg bg-muted/60 p-0.5 ring-1 ring-inset ring-border/40";
  const SEG =
    "flex h-6 items-center gap-1 rounded-md px-2 text-[11px] font-semibold text-muted-foreground transition-colors duration-150 hover:bg-card hover:text-foreground disabled:opacity-40";
  const SEG_ICON =
    "flex size-6 items-center justify-center rounded-md text-muted-foreground transition-colors duration-150 hover:bg-card hover:text-foreground disabled:opacity-40";
  const SEG_ACTIVE =
    "bg-card text-foreground shadow-(--shadow-craft-inset) ring-1 ring-inset ring-border/40";
  const SOLO =
    "flex h-6 items-center gap-1 rounded-md border border-border/40 bg-muted/40 px-2 text-[11px] font-semibold text-muted-foreground transition-colors duration-150 hover:bg-card hover:text-foreground disabled:opacity-40";

  const speedLabel = (s: number) => `${s.toFixed(2).replace(/\.?0+$/, "")}×`;
</script>

<div class="mb-2 flex flex-wrap items-center justify-between gap-2 text-[11px]">
  <!-- EDIT + INSERT -->
  <div class="flex items-center gap-1">
    <InspectorHint content={trimHint} />

    <!-- History -->
    <div class={GROUP}>
      <button
        type="button"
        onclick={() => store.undo()}
        disabled={!store.canUndo}
        title="Undo (Ctrl+Z)"
        aria-label="Undo"
        class={SEG_ICON}
      >
        <Undo2 class="size-3" />
      </button>
      <button
        type="button"
        onclick={() => store.redo()}
        disabled={!store.canRedo}
        title="Redo (Ctrl+Shift+Z)"
        aria-label="Redo"
        class={SEG_ICON}
      >
        <Redo2 class="size-3" />
      </button>
    </div>

    <!-- Edit: split + trim to playhead -->
    <div class={GROUP}>
      <button
        type="button"
        onclick={onSplit}
        title="Split the clip at the playhead (S)"
        class={SEG}
      >
        <SquareSplitHorizontal class="size-3" />
        <span class="hidden sm:inline">Split</span>
        <Kbd class="ml-0.5">S</Kbd>
      </button>
      <button
        type="button"
        onclick={onToggleRazor}
        aria-pressed={razorActive}
        title="Cut tool (C). Click two points to remove a section. Esc to exit."
        class={cn(SEG, razorActive && SEG_ACTIVE)}
      >
        <Scissors class="size-3" />
        <span class="hidden sm:inline">Cut</span>
        <Kbd class="ml-0.5">C</Kbd>
      </button>
      <button
        type="button"
        onclick={() => onSetTrim("in")}
        title="Trim the start to the playhead (I)"
        class={SEG}
      >
        <span class="hidden sm:inline">Start here</span>
        <span class="sm:hidden">Start</span>
        <Kbd class="ml-0.5">I</Kbd>
      </button>
      <button
        type="button"
        onclick={() => onSetTrim("out")}
        title="Trim the end to the playhead (O)"
        class={SEG}
      >
        <span class="hidden sm:inline">End here</span>
        <span class="sm:hidden">End</span>
        <Kbd class="ml-0.5">O</Kbd>
      </button>
    </div>

    {#if hasTrim}
      <button
        type="button"
        onclick={onResetTrim}
        title="Restore the full recording (undo all trims)"
        class={SOLO}
      >
        <Expand class="size-3" />
        <span class="hidden sm:inline">Use full clip</span>
      </button>
    {/if}

    <!-- Insert: focus regions, suggestions, silence removal -->
    <div class={GROUP}>
      <button
        type="button"
        onclick={onAddFocusRegion}
        title="Punch in on the moment at the playhead (zoom region)"
        class={SEG}
      >
        <ZoomIn class="size-3" />
        Zoom
      </button>
      <Popover.Root open={suggestOpen} onOpenChange={(v) => (suggestOpen = v)}>
        <Popover.Trigger>
          {#snippet child({ props })}
            <button
              {...props}
              type="button"
              disabled={!store.cursorPath}
              title={store.cursorPath
                ? "Suggest focus regions from captured cursor activity"
                : "No cursor data in this clip"}
              class={cn(SEG, suggestOpen && SEG_ACTIVE)}
            >
              <AiWand class="size-3" />
              Suggest
            </button>
          {/snippet}
        </Popover.Trigger>
        <Popover.Content
          side="top"
          align="start"
          class="w-auto border-0 bg-transparent p-0 shadow-none ring-0"
        >
          <ZoomSuggestionsPopover {store} onclose={() => (suggestOpen = false)} />
        </Popover.Content>
      </Popover.Root>
    </div>

    <!-- Gated behind the experimental flag (Settings → Experimental): in-progress UI. -->
    {#if experimentalStore.silenceDetection}
      <Popover.Root open={showSilence} onOpenChange={(v) => (showSilence = v)}>
        <Popover.Trigger>
          {#snippet child({ props })}
            <button
              {...props}
              type="button"
              disabled={!store.audioPath && !store.microphonePath}
              title={store.audioPath || store.microphonePath
                ? "Find and remove silent gaps in this recording"
                : "This clip has no audio track to analyse"}
              class={cn(SOLO, showSilence && SEG_ACTIVE)}
            >
              <VolumeX class="size-3" />
              Remove silence
              {#if silenceCutCount > 0}
                <span
                  class="rounded bg-primary/15 px-1 text-[9px] font-bold text-primary"
                >
                  {silenceCutCount}
                </span>
              {/if}
            </button>
          {/snippet}
        </Popover.Trigger>
        <Popover.Content
          side="top"
          align="start"
          class="w-auto border-0 bg-transparent p-0 shadow-none ring-0"
        >
          <SilenceReviewPopover {store} onclose={() => (showSilence = false)} />
        </Popover.Content>
      </Popover.Root>
    {/if}
  </div>

  <!-- VIEW -->
  <div class="flex items-center gap-1.5 text-muted-foreground">
    {#if hasTrim}
      <span
        class="inline-flex h-6 items-center gap-1 rounded-md border border-primary/30 bg-primary/10 px-2 font-mono text-[10px] font-semibold tabular-nums text-primary"
        title="Length of the kept clip"
      >
        <Scissors class="size-2.5" />
        {formatTimeByMode(store.clipDuration, timeMode, fps)}
      </span>
    {/if}

    <!-- Preview rate is a viewing aid, not the export. When it isn't 1x it used
         to be invisible once the View menu closed, which reads as "the export is
         wrong". Persist it as a chip you can click to reset. -->
    {#if playbackSpeed !== 1}
      <button
        type="button"
        onclick={() => onSelectSpeed(1)}
        title="Preview is playing at {speedLabel(
          playbackSpeed,
        )} (viewing only, not the export). Click to reset to 1x."
        class="inline-flex h-6 items-center gap-1 rounded-md border border-border/60 bg-muted/60 px-2 font-mono text-[10px] font-semibold tabular-nums text-foreground ring-1 ring-inset ring-border/40 transition-colors hover:bg-card"
      >
        <FastForward class="size-2.5" />
        {speedLabel(playbackSpeed)}
        <span class="font-sans font-medium text-muted-foreground">preview</span>
      </button>
    {/if}

    <div class={GROUP}>
      <button
        type="button"
        onclick={() => onZoomTimeline(-1)}
        aria-label="Zoom out timeline"
        class={SEG_ICON}
      >
        <Minus class="size-3" />
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
        class={SEG_ICON}
      >
        <Plus class="size-3" />
      </button>
      <button
        type="button"
        onclick={onZoomToFit}
        aria-label="Zoom to fit"
        title="Fit the entire clip in view"
        class={SEG_ICON}
      >
        <Maximize2 class="size-3" />
      </button>
      <button
        type="button"
        onclick={onZoomToSelection}
        disabled={!hasSelectedRegion}
        aria-label="Zoom to selection"
        title={hasSelectedRegion
          ? "Zoom the timeline to fit the selection"
          : "Select a zoom, markup, or cut first"}
        class={SEG_ICON}
      >
        <Target class="size-3" />
      </button>
    </div>

    <DropdownMenu.Root>
      <DropdownMenu.Trigger>
        <button
          type="button"
          aria-label={effectsOff > 0
            ? `Layers (${effectsOff} export effect${effectsOff > 1 ? "s" : ""} off)`
            : "Layers"}
          title={effectsOff > 0
            ? `${effectsOff} export effect${effectsOff > 1 ? "s" : ""} turned off`
            : "Show timeline lanes and choose what to apply on export"}
          class={cn(SOLO, "relative")}
        >
          <Layers class="size-3" />
          <span class="hidden md:inline">Layers</span>
          <!-- Warns that something that changes the output file is switched off. -->
          {#if effectsOff > 0}
            <span
              class="flex size-3.5 items-center justify-center rounded-full bg-lane-cut text-[8px] font-bold leading-none text-background"
            >
              {effectsOff}
            </span>
          {/if}
        </button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Content size="sm" align="end" class="w-56">
        <!-- Cosmetic: which lanes are drawn. Never affects the export. -->
        <DropdownMenu.Label class="flex items-center gap-1.5">
          <Eye class="size-3" />
          Show in timeline
        </DropdownMenu.Label>
        <DropdownMenu.CheckboxItem
          checked={showAudioLane}
          onCheckedChange={onToggleAudioLane}
        >
          <AudioLines class="size-3" />
          Audio
        </DropdownMenu.CheckboxItem>
        <DropdownMenu.CheckboxItem
          checked={showZoomLane}
          onCheckedChange={onToggleZoomLane}
        >
          <ZoomIn class="size-3" />
          Zoom
        </DropdownMenu.CheckboxItem>
        <DropdownMenu.CheckboxItem
          checked={showMarkupLane}
          onCheckedChange={onToggleMarkupLane}
        >
          <Pencil class="size-3" />
          Markup
        </DropdownMenu.CheckboxItem>
        <DropdownMenu.CheckboxItem
          checked={showCutLane}
          onCheckedChange={onToggleCutLane}
        >
          <Scissors class="size-3" />
          Cuts
        </DropdownMenu.CheckboxItem>
        <DropdownMenu.CheckboxItem
          checked={showCutGaps}
          onCheckedChange={onToggleCutGaps}
        >
          <SquareSplitHorizontal class="size-3" />
          Show cut gaps
        </DropdownMenu.CheckboxItem>

        <DropdownMenu.Separator />

        <!-- Functional: what gets baked into the exported file. Worded as verbs
             so it never reads the same as the cosmetic lane rows above. -->
        <DropdownMenu.Label class="flex items-center gap-1.5">
          <Clapperboard class="size-3" />
          Apply on export
        </DropdownMenu.Label>
        <DropdownMenu.CheckboxItem
          checked={store.cutsEnabled}
          onCheckedChange={() => (store.cutsEnabled = !store.cutsEnabled)}
        >
          <Scissors class="size-3" />
          Apply cuts
        </DropdownMenu.CheckboxItem>
        <DropdownMenu.CheckboxItem
          checked={store.focusEnabled}
          onCheckedChange={() => (store.focusEnabled = !store.focusEnabled)}
        >
          <ZoomIn class="size-3" />
          Apply zoom
        </DropdownMenu.CheckboxItem>
        <DropdownMenu.CheckboxItem
          checked={!store.annotationsGloballyHidden}
          onCheckedChange={() =>
            (store.annotationsGloballyHidden = !store.annotationsGloballyHidden)}
        >
          <Pencil class="size-3" />
          Apply markup
        </DropdownMenu.CheckboxItem>
      </DropdownMenu.Content>
    </DropdownMenu.Root>

    <DropdownMenu.Root>
      <DropdownMenu.Trigger>
        <button type="button" aria-label="View options" class={SOLO}>
          <SlidersHorizontal class="size-3" />
          <span class="hidden md:inline">View</span>
        </button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Content size="sm" align="end" class="w-52">
        <!-- Preview rate is a VIEWING aid only (not the export), kept here, away
             from the per-clip Clip speed in the sidebar, so the two never read alike. -->
        <DropdownMenu.Label class="flex items-center gap-1.5">
          <FastForward class="size-3" />
          Preview rate
        </DropdownMenu.Label>
        <DropdownMenu.RadioGroup
          value={String(playbackSpeed)}
          onValueChange={(v) => onSelectSpeed(parseFloat(v))}
        >
          {#each speeds as speed (speed)}
            <DropdownMenu.RadioItem value={String(speed)}>
              {speedLabel(speed)}
            </DropdownMenu.RadioItem>
          {/each}
        </DropdownMenu.RadioGroup>

        <DropdownMenu.Separator />

        <DropdownMenu.Label class="flex items-center gap-1.5">
          <Clock class="size-3" />
          Time display
        </DropdownMenu.Label>
        <DropdownMenu.RadioGroup
          value={timeMode}
          onValueChange={(v) => onSetTimeMode(v as TimeMode)}
        >
          <DropdownMenu.RadioItem value="smpte">Timecode</DropdownMenu.RadioItem>
          <DropdownMenu.RadioItem value="seconds">Seconds</DropdownMenu.RadioItem>
          <DropdownMenu.RadioItem value="frames">Frames</DropdownMenu.RadioItem>
        </DropdownMenu.RadioGroup>

        <DropdownMenu.Separator />

        <DropdownMenu.Label
          class="flex items-center justify-between font-normal text-muted-foreground"
        >
          Aspect ratio
          <span class="font-mono tabular-nums text-foreground">
            {aspectRatioLabel}
          </span>
        </DropdownMenu.Label>
        <DropdownMenu.Label
          class="flex items-center justify-between font-normal text-muted-foreground"
        >
          Frames
          <span class="font-mono tabular-nums text-foreground">
            {frameCount}f
          </span>
        </DropdownMenu.Label>

        <DropdownMenu.Separator />

        <DropdownMenu.Label class="flex items-center gap-1.5">
          <Keyboard class="size-3" />
          Shortcuts
        </DropdownMenu.Label>
        <DropdownMenu.Label
          class="flex items-center justify-between font-normal text-muted-foreground"
        >
          Pan
          <Kbd>Scroll</Kbd>
        </DropdownMenu.Label>
        <DropdownMenu.Label
          class="flex items-center justify-between font-normal text-muted-foreground"
        >
          Zoom
          <Kbd>⌘ Scroll</Kbd>
        </DropdownMenu.Label>
      </DropdownMenu.Content>
    </DropdownMenu.Root>
  </div>
</div>
