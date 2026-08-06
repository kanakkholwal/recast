<script lang="ts">
import {
	DEFAULT_GRADIENT,
	MAX_GRADIENT_STOPS,
	parseGradient,
	serializeGradient,
	type EditorStore,
	type GradientSpec,
} from "../../stores/editor-store.svelte";
import { Move, Plus, RotateCw, Trash2 } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { ColorField } from "@recast/ui/color-field";
import { SliderControl } from "@recast/ui/slider-control";
import { cn } from "@recast/ui/utils";
import {
	clampStopPos,
	insertStopInWidestGap,
	posFromPointerX,
	sampleStopColor,
} from "./background-picker.logic";
import PanelSection from "./PanelSection.svelte";

interface Props {
	store: EditorStore;
	/** Shared recent-colors list (also used by the color/shadow pickers). */
	recents: string[];
	/** Record a freshly-picked color into the shared recents. */
	onRememberColor: (color: string) => void;
}

let { store, recents, onRememberColor }: Props = $props();

// Local editing draft so dragging a stop doesn't round-trip through the store
// every pointer-move; streamed back via setBackgroundLive (coalesced undo). It
// serialises to the same CSS string both renderers (preview + Rust export) parse.
let gradientDraft = $derived<GradientSpec>(
	parseGradient(store.backgroundType === "gradient" ? store.backgroundValue : DEFAULT_GRADIENT),
);
let selectedStop = $state(0);
let gradientBarEl = $state<HTMLDivElement | null>(null);

// Reconcile the draft on outside changes (undo/redo, preset click). The
// serialise-compare stops our own live edits from bouncing back into the drag.
$effect(() => {
	if (store.backgroundType !== "gradient") return;
	const current = store.backgroundValue;
	if (current !== serializeGradient(gradientDraft)) {
		gradientDraft = parseGradient(current);
		if (selectedStop >= gradientDraft.stops.length) selectedStop = 0;
	}
});

const gradientCss = $derived(serializeGradient(gradientDraft));

// Live commit (drag gestures) → single coalesced undo entry. Discrete edits
// (add/remove stop) pass `live=false` for a clean, individually-undoable step.
function commitGradient(next: GradientSpec, live = true) {
	gradientDraft = next;
	const value = serializeGradient(next);
	if (live) store.setBackgroundLive("gradient", value);
	else store.setBackground({ type: "gradient", value });
}

function setStopColor(i: number, color: string) {
	commitGradient({
		...gradientDraft,
		stops: gradientDraft.stops.map((s, j) => (j === i ? { ...s, color } : s)),
	});
}

function setStopPos(i: number, pos: number) {
	const clamped = clampStopPos(pos);
	commitGradient({
		...gradientDraft,
		stops: gradientDraft.stops.map((s, j) => (j === i ? { ...s, pos: clamped } : s)),
	});
}

function setAngle(angle: number) {
	commitGradient({ ...gradientDraft, angle });
}

// Wrapper: samples the reactive draft's stops via the shared sRGB helper.
const sampleDraftColor = (pos: number): string => sampleStopColor(gradientDraft.stops, pos);

function addStop() {
	if (gradientDraft.stops.length >= MAX_GRADIENT_STOPS) return;
	const stops = [...gradientDraft.stops, insertStopInWidestGap(gradientDraft.stops)];
	commitGradient({ ...gradientDraft, stops }, false);
	selectedStop = stops.length - 1;
}

function removeStop(i: number) {
	if (gradientDraft.stops.length <= 2) return;
	const stops = gradientDraft.stops.filter((_, j) => j !== i);
	commitGradient({ ...gradientDraft, stops }, false);
	selectedStop = Math.min(selectedStop, stops.length - 1);
}

// Drag a stop handle along the bar. Streams position live; the whole drag
// coalesces to one undo entry via `setBackgroundLive`.
function startStopDrag(e: PointerEvent, i: number) {
	e.preventDefault();
	selectedStop = i;
	const bar = gradientBarEl;
	if (!bar) return;
	const rect = bar.getBoundingClientRect();
	const move = (ev: PointerEvent) => {
		setStopPos(i, posFromPointerX(ev.clientX, rect));
	};
	const up = () => {
		window.removeEventListener("pointermove", move);
		window.removeEventListener("pointerup", up);
	};
	window.addEventListener("pointermove", move);
	window.addEventListener("pointerup", up);
}

// Double-click an empty spot on the bar to drop a new stop there.
function addStopAtPointer(e: MouseEvent) {
	if (gradientDraft.stops.length >= MAX_GRADIENT_STOPS) return;
	const bar = gradientBarEl;
	if (!bar) return;
	const rect = bar.getBoundingClientRect();
	const pos = clampStopPos(posFromPointerX(e.clientX, rect));
	const stops = [...gradientDraft.stops, { color: sampleDraftColor(pos), pos }];
	commitGradient({ ...gradientDraft, stops }, false);
	selectedStop = stops.length - 1;
}
</script>

<!-- Custom gradient builder: drag stops, double-click to add, edit the
     selected stop's color/position/angle. Edits stream live and coalesce
     into one undo step per gesture. -->
<PanelSection
  title="Custom"
  hint="Drag stops to reposition · double-click the bar to add a stop."
  flush
>
  {#snippet action()}
    <Button
      variant="ghost"
      size="xs"
      class="h-6 gap-1 px-1.5 text-[10.5px] text-muted-foreground"
      onclick={addStop}
      disabled={gradientDraft.stops.length >= MAX_GRADIENT_STOPS}
    >
      <Plus size={11} />
      Add stop
    </Button>
  {/snippet}

  <div class="flex flex-col gap-2.5">
    <div
      bind:this={gradientBarEl}
      ondblclick={addStopAtPointer}
      role="presentation"
      class="relative h-9 w-full overflow-visible rounded-md border border-border/60 shadow-(--shadow-craft-inset)"
      style="background: {gradientCss}"
    >
      {#each gradientDraft.stops as stop, i (i)}
        <button
          type="button"
          onpointerdown={(e) => startStopDrag(e, i)}
          onclick={() => (selectedStop = i)}
          ondblclick={(e) => e.stopPropagation()}
          class={cn(
            "absolute top-1/2 size-4 -translate-x-1/2 -translate-y-1/2 cursor-grab rounded-full border-2 shadow-md transition-transform active:cursor-grabbing",
            i === selectedStop
              ? "scale-110 border-primary ring-2 ring-primary/40"
              : "border-white/90 hover:scale-105",
          )}
          style="left: {stop.pos}%; background-color: {stop.color}"
          aria-label="Gradient stop {i + 1} at {Math.round(stop.pos)}%"
          aria-pressed={i === selectedStop}
        ></button>
      {/each}
    </div>

    <div class="flex items-center gap-1.5">
      <div class="min-w-0 flex-1">
        <ColorField
          label="Stop {selectedStop + 1}"
          value={gradientDraft.stops[selectedStop]?.color ?? "#000000"}
          {recents}
          allowAlpha={false}
          oncommit={(c: string) => {
            setStopColor(selectedStop, c);
            onRememberColor(c);
          }}
        />
      </div>
      <Button
        variant="ghost"
        size="icon-sm"
        class="shrink-0 text-muted-foreground hover:text-destructive"
        onclick={() => removeStop(selectedStop)}
        disabled={gradientDraft.stops.length <= 2}
        aria-label="Remove selected stop"
      >
        <Trash2 size={13} />
      </Button>
    </div>

    <SliderControl
      label="Position"
      value={gradientDraft.stops[selectedStop]?.pos ?? 0}
      min={0}
      max={100}
      step={1}
      unit="%"
      onstart={() => {}}
      onchange={(v) => setStopPos(selectedStop, v)}
    >
      {#snippet icon()}
        <Move size={11} />
      {/snippet}
    </SliderControl>

    <SliderControl
      label="Angle"
      value={gradientDraft.angle}
      min={0}
      max={360}
      step={1}
      unit="°"
      onstart={() => {}}
      onchange={(v) => setAngle(v)}
    >
      {#snippet icon()}
        <RotateCw size={11} />
      {/snippet}
    </SliderControl>
  </div>
</PanelSection>
