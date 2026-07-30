<script lang="ts">
import { bezierY, easingEquals, type Easing } from "$lib/easing/cubic-bezier";
import { registry } from "$lib/registry";
import { SegmentedToggle } from "@recast/ui/segmented";
import { cn } from "@recast/ui/utils";
import BezierEditor from "../_components/BezierEditor.svelte";
import PanelSection from "./PanelSection.svelte";

// The one easing control. Intent-named presets lead, a live preview shows what
// they feel like, and the raw curve stays behind a disclosure. Both zoom and
// annotations used to hand-roll the in/out switch plus graph around
// BezierEditor, and annotations shipped no presets at all.
interface Props {
	/** One curve, or both ramps when a feature eases in and out separately. */
	value: Easing | { in: Easing; out: Easing };
	/** Preset pick. Applies to every curve in `value`; push undo in the handler. */
	onpick: (next: Easing) => void;
	/** Graph drag on the ramp being edited. Coalesce undo in the handler. */
	ondrag: (next: Easing, which: "in" | "out" | null) => void;
	size?: number;
}

let { value, onpick, ondrag, size = 200 }: Props = $props();

const isPair = $derived(typeof value === "object" && "in" in value);
// Which ramp the graph edits. Panel-local state that belonged here, not in
// every caller.
let ramp = $state<"in" | "out">("in");
const active = $derived(isPair ? (value as { in: Easing; out: Easing })[ramp] : (value as Easing));

// From the registry, so easing added by an extension pack surfaces here too.
const presets = $derived(
	registry.list("easing").map((e) => ({ id: e.id, label: e.label, value: e.value.value })),
);

// In pair mode a preset only counts as active when BOTH ramps use it, otherwise
// picking it would silently change the ramp you are not looking at.
function presetActive(p: Easing): boolean {
	if (!isPair) return easingEquals(active, p);
	const v = value as { in: Easing; out: Easing };
	return easingEquals(v.in, p) && easingEquals(v.out, p);
}

const activeIndex = $derived(
	Math.max(
		0,
		presets.findIndex((p) => presetActive(p.value)),
	),
);
let chips = $state<(HTMLButtonElement | null)[]>([]);

function moveTo(index: number) {
	const preset = presets[(index + presets.length) % presets.length];
	if (!preset) return;
	onpick({ ...preset.value });
	chips[(index + presets.length) % presets.length]?.focus();
}

function handleChipKeys(e: KeyboardEvent) {
	switch (e.key) {
		case "ArrowRight":
		case "ArrowDown":
			moveTo(activeIndex + 1);
			break;
		case "ArrowLeft":
		case "ArrowUp":
			moveTo(activeIndex - 1);
			break;
		case "Home":
			moveTo(0);
			break;
		case "End":
			moveTo(presets.length - 1);
			break;
		default:
			return;
	}
	e.preventDefault();
}

// Looping playhead for the preview. rAF stops on its own when the window is
// hidden, and `t` is written from the callback so the effect depends only on the
// reduced-motion flag, never on the curve being dragged.
const PREVIEW_MS = 1400;
let reduced = $state(false);
let t = $state(0);

$effect(() => {
	if (typeof window === "undefined") return;
	const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
	reduced = mq.matches;
	const on = (e: MediaQueryListEvent) => (reduced = e.matches);
	mq.addEventListener("change", on);
	return () => mq.removeEventListener("change", on);
});

$effect(() => {
	if (reduced) {
		t = 1;
		return;
	}
	let raf = 0;
	let start = 0;
	const loop = (now: number) => {
		if (!start) start = now;
		t = ((now - start) % PREVIEW_MS) / PREVIEW_MS;
		raf = requestAnimationFrame(loop);
	};
	raf = requestAnimationFrame(loop);
	return () => cancelAnimationFrame(raf);
});

const progress = $derived(bezierY(active, t));
</script>

<div class="flex flex-col gap-2">
	<!-- Wrapped chips rather than a 7-way Segmented: "Ease In Out" does not
	     survive a seventh of a 280px inspector. Radiogroup semantics + roving
	     tabindex give it the arrow keys a Segmented would have. -->
	<div
		role="radiogroup"
		tabindex="-1"
		aria-label="Easing preset"
		class="flex flex-wrap gap-1"
		onkeydown={handleChipKeys}
	>
		{#each presets as preset, i (preset.id)}
			{@const checked = presetActive(preset.value)}
			<button
				bind:this={chips[i]}
				type="button"
				role="radio"
				aria-checked={checked}
				tabindex={i === activeIndex ? 0 : -1}
				onclick={() => onpick({ ...preset.value })}
				class={cn(
					"h-6 rounded-md border px-2 text-[10px] font-medium transition-colors",
					"focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-ring",
					checked
						? "border-primary/60 bg-primary/10 text-primary"
						: "border-border/60 bg-card/60 text-muted-foreground hover:border-border hover:text-foreground",
				)}
			>
				{preset.label}
			</button>
		{/each}
	</div>

	<!-- Feel the curve without opening the graph: a dot on a track, retimed by
	     the active easing. aria-hidden because it reports no state a screen
	     reader needs, and it never stops moving. -->
	<div
		aria-hidden="true"
		class="relative h-5 overflow-hidden rounded-md border border-border/50 bg-card/40"
	>
		<!-- Inset to 80% of the track so Bounce's overshoot and undershoot stay on
		     screen instead of being clipped at the ends. -->
		<div
			class="absolute top-1/2 size-2 -translate-x-1/2 -translate-y-1/2 rounded-full bg-primary"
			style="left: calc({(progress * 100).toFixed(2)}% * 0.8 + 10%)"
		></div>
	</div>

	{#if isPair}
		<div class="flex items-center justify-between gap-2">
			<span class="text-[10px] text-muted-foreground">
				Graph edits the ease-{ramp} ramp
			</span>
			<SegmentedToggle
				checked={ramp === "out"}
				offLabel="In"
				onLabel="Out"
				size="xs"
				aria-label="Edit the ease-in or ease-out curve"
				onCheckedChange={(next) => (ramp = next ? "out" : "in")}
			/>
		</div>
	{/if}

	<PanelSection title="Custom curve" flush collapsible defaultOpen={false}>
		<div class="pt-1">
			<BezierEditor
				value={active}
				onchange={(next) => ondrag(next, isPair ? ramp : null)}
				{size}
			/>
		</div>
	</PanelSection>
</div>
