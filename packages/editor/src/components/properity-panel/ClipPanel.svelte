<script lang="ts">
import {
	ArrowDown,
	ArrowLeft,
	ArrowRight,
	ArrowUp,
	RotateCcw,
	SquareSplitHorizontal,
	Trash2,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import { Segmented } from "@recast/ui/segmented";
import { clockCentis } from "../../lib/format/time";
import type { SeamTransition } from "../../lib/scenes/seam";
import type { MotionTone } from "../../lib/scenes/segment-anim";
import { MAX_SEGMENT_SPEED, MIN_SEGMENT_SPEED } from "../../lib/timeline/segment-speed";
import type { EditorStore } from "../../stores/editor-store.svelte";
import { anchorMatches, fmtSpeed } from "./clip-panel.logic";
import PanelSection from "./PanelSection.svelte";
import PropRow from "./PropRow.svelte";
import PropSelect from "./PropSelect.svelte";
import SceneAnimControls from "./SceneAnimControls.svelte";
import SliderRow from "./SliderRow.svelte";

// Contextual controls for the clip/segment selected on the timeline. Auto-opened
// by PropertiesPanel when `selectedClipStart` is set (mirrors the Focus tab for
// zoom regions). Speed writes go through `store.setSegmentSpeed` (coalesced undo).

interface Props {
	store: EditorStore;
}
let { store }: Props = $props();

const SPEED_PRESETS = [0.5, 1, 1.5, 2];

// Project-wide scene-animation motion style (restyles every clip's animation).
const MOTION_TONES = [
	{ id: "subtle", label: "Subtle" },
	{ id: "balanced", label: "Balanced" },
	{ id: "energetic", label: "Energetic" },
] as const;

// The selected kept segment, matched by its original start anchor.
const selected = $derived.by(() => {
	const start = store.selectedClipStart;
	if (start === null) return null;
	return store.segments.find((s) => anchorMatches(s.start, start)) ?? null;
});
const speed = $derived(selected ? store.segmentSpeedAt(selected.start) : 1);
const isSped = $derived(!anchorMatches(speed, 1));
// "" when the fine slider has landed off-preset, so no segment reads as active.
const activeSpeedPreset = $derived(
	String(SPEED_PRESETS.find((p) => anchorMatches(speed, p)) ?? ""),
);
const speedPresetOptions = $derived([
	...SPEED_PRESETS.map((p) => ({ value: String(p), label: fmtSpeed(p) })),
	...(activeSpeedPreset ? [] : [{ value: "custom", label: "Custom" }]),
]);

// A seam sits before this clip when a cut removed content between it and the
// previous segment. That's where a transition smooths the jump.
const prevSeg = $derived(
	selected && selected.index > 0
		? (store.segments.find((s) => s.index === selected.index - 1) ?? null)
		: null,
);
const seamBefore = $derived(
	prevSeg !== null && selected !== null && selected.start - prevSeg.end > 1e-4,
);
const seamKind = $derived(
	seamBefore && prevSeg && selected
		? store.seamTransitionAt(prevSeg.start, selected.start)
		: "none",
);
function setSeam(kind: SeamTransition) {
	if (prevSeg && selected) store.setSeamTransition(prevSeg.start, selected.start, kind);
}

function setSpeed(v: number) {
	if (selected) store.setSegmentSpeed(selected.start, v);
}
function splitHere() {
	store.splitAt(store.currentTime);
}
function deleteClip() {
	if (!selected) return;
	const joinAt = store.deleteSegmentAt((selected.start + selected.end) / 2);
	if (joinAt !== null) store.seek(joinAt);
}
</script>

{#if !selected}
  <div
    class="flex flex-col items-center justify-center gap-2 px-3 py-12 text-center animate-in fade-in duration-200"
  >
    <SquareSplitHorizontal class="size-6 text-muted-foreground/50" />
    <p class="text-[11px] leading-snug text-muted-foreground">
      Select a clip on the timeline to change its speed, split it, or remove it.
    </p>
  </div>
{:else}
  {@const duration = selected.end - selected.start}
  <div class="space-y-3 animate-in fade-in duration-200">
    <div class="rounded-lg bg-muted/30 px-3 py-2 ring-1 ring-inset ring-border/40">
      <div class="flex items-baseline justify-between">
        <span class="text-[11px] text-muted-foreground">Clip duration</span>
        <span class="font-mono text-[12px] tabular-nums text-foreground">
          {clockCentis(duration)}
        </span>
      </div>
      {#if isSped}
        <div class="mt-0.5 flex items-baseline justify-between text-[10px] text-muted-foreground">
          <span>Plays in</span>
          <span class="font-mono tabular-nums text-foreground">
            {clockCentis(duration / speed)} at {fmtSpeed(speed)}
          </span>
        </div>
      {/if}
    </div>

    <PanelSection
      title="Clip speed"
      hint="How fast this clip plays, in both preview and export. 1× is normal."
    >
      {#snippet action()}
        {#if isSped}
          <Button
            variant="ghost"
            size="xs"
            class="gap-1 text-[10.5px] text-muted-foreground"
            onclick={() => setSpeed(1)}
            title="Reset this clip to 1×"
          >
            <RotateCcw class="size-2.5" />
            Reset
          </Button>
        {/if}
      {/snippet}
      <div class="space-y-1.5">
        <PropRow label="Preset">
          <PropSelect
            class="flex-1"
            label="Clip speed preset"
            value={activeSpeedPreset || "custom"}
            options={speedPresetOptions}
            onChange={(v) => {
              if (v !== "custom") setSpeed(Number(v));
            }}
          />
        </PropRow>
        <SliderRow
          label="Speed"
          value={speed}
          min={MIN_SEGMENT_SPEED}
          max={MAX_SEGMENT_SPEED}
          step={0.05}
          formatValue={(v) => `${v.toFixed(2)}×`}
          onchange={(v) => setSpeed(v)}
        />
      </div>
    </PanelSection>

    {#if seamBefore}
      <PanelSection
        title="Cut transition"
        hint="Smooth the jump where a cut removed content: this clip pushes in as the previous one pushes out."
      >
        <!-- Two rows: mode, then direction. Six segments in one row truncates
             "None"/"Dip" at panel width, and `Segmented` cannot wrap. Only one
             row shows a selection at a time — the other matches no option and
             renders no pill. -->
        <div class="flex flex-col gap-1.5">
          <PropRow label="Mode">
            <Segmented
              size="xs"
              aria-label="Cut transition"
              value={seamKind}
              options={[
                { value: "none", label: "None", title: "No transition" },
                { value: "dip", label: "Dip", title: "Dip to background" },
              ]}
              onValueChange={(v) => setSeam(v as SeamTransition)}
            />
          </PropRow>
          <PropRow label="Direction">
            <div class="contents">
              {#snippet icoLeft()}<ArrowLeft class="size-3.5" />{/snippet}
              {#snippet icoRight()}<ArrowRight class="size-3.5" />{/snippet}
              {#snippet icoUp()}<ArrowUp class="size-3.5" />{/snippet}
              {#snippet icoDown()}<ArrowDown class="size-3.5" />{/snippet}
              <Segmented
                size="xs"
                aria-label="Push direction"
                value={seamKind}
                options={[
                  { value: "push-left", icon: icoLeft, title: "Push left" },
                  { value: "push-right", icon: icoRight, title: "Push right" },
                  { value: "push-up", icon: icoUp, title: "Push up" },
                  { value: "push-down", icon: icoDown, title: "Push down" },
                ]}
                onValueChange={(v) => setSeam(v as SeamTransition)}
              />
            </div>
          </PropRow>
        </div>
        {#if seamKind === "custom"}
          <p class="mt-1 text-[10px] text-muted-foreground/70">
            Custom entrance/exit set on these clips. Pick a push to replace it.
          </p>
        {/if}
      </PanelSection>
    {/if}

    <PanelSection
      title="Scene animation"
      hint="How this clip animates into and out of view. Applies to the video layer only, in both preview and export."
    >
      <div class="space-y-3">
        <div class="space-y-1">
          <PropRow label="Motion">
            <PropSelect
              class="flex-1"
              label="Scene animation motion style"
              value={store.motionTone}
              options={MOTION_TONES.map((t) => ({ value: t.id, label: t.label }))}
              onChange={(v) => store.setMotionTone(v as MotionTone)}
            />
          </PropRow>
          <p class="text-[10px] text-muted-foreground/70">Applies to all clips.</p>
        </div>
        <div class="space-y-1.5">
          <span class="text-[11px] text-muted-foreground">Entrance</span>
          <SceneAnimControls {store} start={selected.start} side="in" />
        </div>
        <div class="space-y-1.5">
          <span class="text-[11px] text-muted-foreground">Exit</span>
          <SceneAnimControls {store} start={selected.start} side="out" />
        </div>
      </div>
    </PanelSection>

    <div class="space-y-1.5">
      <Button
        variant="outline"
        size="sm"
        class="w-full justify-start gap-2"
        onclick={splitHere}
      >
        <SquareSplitHorizontal class="size-3.5" />
        Split at playhead
      </Button>
      <Button
        variant="destructive_soft"
        size="sm"
        class="w-full justify-start gap-2"
        disabled={store.segments.length <= 1}
        title={store.segments.length <= 1
          ? "The only clip can't be deleted"
          : "Delete this clip"}
        onclick={deleteClip}
      >
        <Trash2 class="size-3.5" />
        Delete clip
      </Button>
    </div>
  </div>
{/if}
