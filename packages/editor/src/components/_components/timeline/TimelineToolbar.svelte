<script lang="ts">
import {
	AiWand,
	AudioLines,
	Clapperboard,
	Clock,
	Expand,
	Eye,
	FastForward,
	Keyboard,
	Layers,
	Maximize2,
	Minus,
	Pencil,
	Plus,
	Scissors,
	SlidersHorizontal,
	SquareSplitHorizontal,
	Target,
	VolumeX,
	ZoomIn,
} from "@recast/icons";
import * as DropdownMenu from "@recast/ui/dropdown-menu";
import { Kbd } from "@recast/ui/kbd";
import * as Popover from "@recast/ui/popover";
import { cn } from "@recast/ui/utils";
import type { EditorStore } from "../../../stores/editor-store.svelte";
import { experimentalStore } from "../../../stores/experimental.svelte";
import InspectorHint from "../../InspectorHint.svelte";
import SilenceReviewPopover from "../../SilenceReviewPopover.svelte";
import ZoomSuggestionsPopover from "../../ZoomSuggestionsPopover.svelte";
import { formatTimeByMode, type TimeMode } from "./timeline-helpers";

// Three clusters (edit, insert, view); popovers are portalled because the timeline's overflow-hidden wrapper would clip them.

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

const trimHint =
	"Trim start / Trim end keep the middle. Cut removes a section between two clicks. Split breaks the clip at the playhead so you can delete or re-speed one piece.";

// `splitAt` already refuses a split that can't land, but the caller discarded that, so the button silently did nothing.
const canSplit = $derived(store.canSplitAt(store.currentTime));
const splitTitle = $derived(
	canSplit
		? "Split the clip at the playhead (S)"
		: "Can't split here: the playhead is on a clip edge, an existing split, or inside a removed section",
);

let suggestOpen = $state(false);
let showSilence = $state(false);

// Counts only silence-detected cuts; manual ripple deletes shouldn't inflate this.
const silenceCutCount = $derived(store.cuts.filter((c) => c.source === "silence").length);

// A badge for export-affecting effects that are off, so 'my cuts didn't apply' is visible without opening the menu.
const effectsOff = $derived(
	(store.cutsEnabled ? 0 : 1) +
		(store.focusEnabled ? 0 : 1) +
		(store.annotationsGloballyHidden ? 1 : 0),
);

// Flat, quiet segments with hairline separators rather than boxed trays.
const GROUP = "flex items-center gap-0.5";
const SEG =
	"flex h-6 items-center gap-1.5 rounded-md px-2 text-[11px] font-medium text-muted-foreground transition-colors duration-150 hover:bg-muted/70 hover:text-foreground disabled:opacity-40";
const SEG_ICON =
	"flex size-6 items-center justify-center rounded-md text-muted-foreground transition-colors duration-150 hover:bg-muted/70 hover:text-foreground disabled:opacity-40";
const SEG_ACTIVE = "bg-foreground/10 text-foreground";
const SOLO = SEG;

const speedLabel = (s: number) => `${s.toFixed(2).replace(/\.?0+$/, "")}×`;
</script>

<div class="mb-2 flex flex-wrap items-center justify-between gap-2 text-[11px]">
  <!-- EDIT + INSERT -->
  <div class="flex items-center gap-1">
    <InspectorHint content={trimHint} />

    <!-- Edit: split + trim to playhead. Undo/redo lives in the app toolbar, not
         here. Shortcuts live in tooltips and the View
         menu's Shortcuts list, never as chips on the buttons themselves. -->
    <div class={GROUP}>
      <button
        type="button"
        onclick={onSplit}
        disabled={!canSplit}
        aria-label="Split at playhead"
        title={splitTitle}
        class={SEG}
      >
        <SquareSplitHorizontal class="size-3" />
        <span class="hidden sm:inline">Split</span>
      </button>
      <button
        type="button"
        onclick={onToggleRazor}
        aria-pressed={razorActive}
        aria-label="Cut tool"
        title="Cut tool (C). Click two points to remove a section. Esc to exit."
        class={cn(SEG, razorActive && SEG_ACTIVE)}
      >
        <Scissors class="size-3" />
        <span class="hidden sm:inline">Cut</span>
      </button>
      <!-- "Trim start", not "Start here": the properties panels use "Start here"
           for moving a zoom/markup region's own edge to the playhead, and these
           two trim the whole clip. -->
      <button
        type="button"
        onclick={() => onSetTrim("in")}
        aria-label="Trim clip start to playhead"
        title="Trim the clip's start to the playhead (I)"
        class={SEG}
      >
        <span class="hidden sm:inline">Trim start</span>
        <span class="sm:hidden">Trim in</span>
      </button>
      <button
        type="button"
        onclick={() => onSetTrim("out")}
        aria-label="Trim clip end to playhead"
        title="Trim the clip's end to the playhead (O)"
        class={SEG}
      >
        <span class="hidden sm:inline">Trim end</span>
        <span class="sm:hidden">Trim out</span>
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

    <div class="mx-1 h-4 w-px bg-border/60" role="separator"></div>

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
      <!-- No scissors here: in this toolbar that icon already means the Cut tool,
           the Cuts lane, and Apply cuts. This is the kept length after trimming. -->
      <span
        class="inline-flex h-6 items-center rounded-md border border-primary/30 bg-primary/10 px-2 font-mono text-[10px] font-semibold tabular-nums text-primary"
        title="Length of the trimmed clip"
      >
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

    <div class="mx-0.5 h-4 w-px bg-border/60" role="separator"></div>

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
