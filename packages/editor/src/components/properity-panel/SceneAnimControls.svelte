<script lang="ts">
import type { IconComponent } from "@recast/icons";
import { ArrowDown, ArrowLeft, ArrowRight, ArrowUp } from "@recast/icons";
import { cn } from "@recast/ui/utils";
import { EASING_PRESETS, easingEquals } from "../../lib/easing/cubic-bezier";
import {
	defaultSpec,
	intensityRange,
	MAX_ANIM_MS,
	MIN_ANIM_MS,
	type SceneAnimDir,
	type SceneAnimKind,
	type SceneAnimSpec,
} from "../../lib/scenes/segment-anim";
import type { EditorStore } from "../../stores/editor-store.svelte";
import PropRow from "./PropRow.svelte";
import PropSelect from "./PropSelect.svelte";
import SliderRow from "./SliderRow.svelte";

// One side (entrance or exit) of a segment's scene animation. Reads/writes the
// spec through `store.setSegmentAnim` (coalesced undo). Kept dumb: all state is
// the store's; this only maps controls onto a SceneAnimSpec.

interface Props {
	store: EditorStore;
	/** Original start anchor of the selected segment. */
	start: number;
	side: "in" | "out";
}
let { store, start, side }: Props = $props();

const KINDS: { id: SceneAnimKind; label: string }[] = [
	{ id: "fade", label: "Fade" },
	{ id: "slide", label: "Slide" },
	{ id: "scale", label: "Scale" },
	{ id: "shrink", label: "Shrink" },
	{ id: "pop", label: "Pop" },
	{ id: "rotate", label: "Rotate" },
];
const DIRS: { id: SceneAnimDir; icon: IconComponent }[] = [
	{ id: "left", icon: ArrowLeft },
	{ id: "right", icon: ArrowRight },
	{ id: "up", icon: ArrowUp },
	{ id: "down", icon: ArrowDown },
];

const spec = $derived<SceneAnimSpec | null>(store.segmentAnimAt(start)?.[side] ?? null);
const range = $derived(spec ? intensityRange(spec.kind) : null);
const intensityValue = $derived(spec?.intensity ?? range?.default ?? 0);

function write(next: SceneAnimSpec | null) {
	store.setSegmentAnim(start, side, next);
}
function pickKind(kind: SceneAnimKind) {
	const base = defaultSpec(kind, side, store.motionTone);
	// Keep the user's tuning (duration/easing) when swapping the kind.
	write({
		...base,
		durationMs: spec?.durationMs ?? base.durationMs,
		easing: spec?.easing ?? base.easing,
	});
}
function patch(part: Partial<SceneAnimSpec>) {
	if (spec) write({ ...spec, ...part });
}

// Easing as a select, matching every other preset picker in the inspector.
const activeEasingId = $derived(
	spec
		? (EASING_PRESETS.find((p) => easingEquals(spec.easing, p.value))?.id ?? "custom")
		: "custom",
);
const easingOptions = $derived([
	...EASING_PRESETS.map((p) => ({ value: p.id, label: p.label })),
	...(activeEasingId === "custom" ? [{ value: "custom", label: "Custom" }] : []),
]);
function pickEasing(id: string) {
	const preset = EASING_PRESETS.find((p) => p.id === id);
	if (preset) patch({ easing: preset.value });
}

const kindOptions = [
	{ value: "off", label: "Off" },
	...KINDS.map((k) => ({ value: k.id, label: k.label })),
];
function pickType(id: string) {
	if (id === "off") write(null);
	else pickKind(id as SceneAnimKind);
}
</script>

<div class="space-y-2">
  <PropRow label="Type">
    <PropSelect
      class="flex-1"
      label="Scene animation"
      value={spec === null ? "off" : spec.kind}
      options={kindOptions}
      onChange={pickType}
    />
  </PropRow>

  {#if spec}
    {#if spec.kind === "slide"}
      <PropRow label="From">
        {#each DIRS as d (d.id)}
          {@const active = (spec.dir ?? "left") === d.id}
          <button
            type="button"
            aria-pressed={active}
            onclick={() => patch({ dir: d.id })}
            class={cn(
              "grid size-7 place-items-center rounded-md border transition-colors",
              active
                ? "border-transparent bg-foreground text-background"
                : "border-border/60 bg-card/40 text-muted-foreground hover:border-border hover:text-foreground",
            )}
          >
            <d.icon size={12} />
          </button>
        {/each}
      </PropRow>
    {/if}

    {#if range}
      <SliderRow
        label={range.label}
        value={intensityValue}
        min={range.min}
        max={range.max}
        step={range.step}
        unit={range.unit}
        formatValue={(v) =>
          range.unit === "°" ? `${Math.round(v)}°` : `${v.toFixed(2)}${range.unit}`}
        onchange={(v) => patch({ intensity: v })}
      />
    {/if}

    <SliderRow
      label="Duration"
      value={spec.durationMs}
      min={MIN_ANIM_MS}
      max={MAX_ANIM_MS}
      step={50}
      formatValue={(v) => `${Math.round(v)}ms`}
      onchange={(v) => patch({ durationMs: v })}
    />

    <PropRow label="Curve">
      <PropSelect
        class="flex-1"
        label="Easing preset"
        value={activeEasingId}
        options={easingOptions}
        onChange={pickEasing}
      />
    </PropRow>
  {/if}
</div>
