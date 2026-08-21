<script lang="ts">
import { prefersReducedMotion } from "$lib/motion-core";
import { Camera, Mic, Video } from "@recast/icons";
import { cn } from "@recast/ui/utils";

// Region capture, shown as the interaction rather than as a menu of it. A
// selection is dragged across a mock desktop, snaps to a size readout, and the
// capture bar rises under it. A command palette listing "Record region" tells
// you the feature exists; this shows what it feels like.
//
// Geometry is percentages of the stage so the mock scales with its container.
// One timeline drives everything, so the marquee, the readout and the bar can
// never disagree about which phase they are in.
type Phase = "idle" | "dragging" | "armed" | "recording";

const reduced = $derived(prefersReducedMotion());

// Cumulative ms at which each phase begins, then loops.
const TIMELINE: Array<{ phase: Phase; at: number }> = [
	{ phase: "idle", at: 0 },
	{ phase: "dragging", at: 600 },
	{ phase: "armed", at: 1900 },
	{ phase: "recording", at: 3100 },
];
const LOOP_MS = 6200;

let elapsed = $state(0);

$effect(() => {
	if (reduced) return;
	const id = setInterval(() => {
		if (!document.hidden) elapsed = (elapsed + 40) % LOOP_MS;
	}, 40);
	return () => clearInterval(id);
});

const phase = $derived.by<Phase>(() => {
	if (reduced) return "armed";
	let current: Phase = "idle";
	for (const step of TIMELINE) if (elapsed >= step.at) current = step.phase;
	return current;
});

// Drag progress 0..1 across the dragging window, eased so the marquee decelerates
// into its final size instead of stopping dead.
const drag = $derived.by(() => {
	if (reduced || phase === "armed" || phase === "recording") return 1;
	if (phase === "idle") return 0;
	const t = (elapsed - 600) / (1900 - 600);
	return 1 - (1 - Math.min(1, Math.max(0, t))) ** 3;
});

// Selection box, in stage percentages.
const X = 14;
const Y = 18;
const W = 60;
const H = 54;

const boxW = $derived(W * drag);
const boxH = $derived(H * drag);
const shown = $derived(drag > 0.02);

// Readout tracks the box so the numbers mean something.
const px = $derived({
	w: Math.round(1280 * (boxW / W || 0)),
	h: Math.round(720 * (boxH / H || 0)),
});

const elapsedLabel = $derived.by(() => {
	if (phase !== "recording") return "00:00";
	const s = Math.floor((elapsed - 3100) / 1000);
	return `00:0${Math.max(0, s)}`;
});
</script>

<div class="p-4">
	<!-- Mock desktop. Abstract window shapes, not a screenshot: it has to read at
	     a glance and never age with the product's real UI. -->
	<div class="stage relative aspect-16/10 w-full overflow-hidden rounded-xl border border-border-low bg-paper">
		<div aria-hidden="true" class="absolute inset-0 p-3">
			<div class="flex h-full gap-2">
				<div class="h-full w-1/4 rounded-lg border border-border-low bg-card"></div>
				<div class="flex h-full flex-1 flex-col gap-2">
					<div class="h-1/3 rounded-lg border border-border-low bg-card"></div>
					<div class="flex-1 rounded-lg border border-border-low bg-card"></div>
				</div>
			</div>
		</div>

		<!-- Dimmed everywhere except the selection. Four panels rather than a
		     ring, so the cut-out edge stays crisp at any size. -->
		{#if shown}
			<div aria-hidden="true" class="absolute inset-0">
				<div class="scrim" style={`left:0;top:0;right:0;height:${Y}%`}></div>
				<div class="scrim" style={`left:0;top:${Y}%;width:${X}%;height:${boxH}%`}></div>
				<div
					class="scrim"
					style={`left:${X + boxW}%;top:${Y}%;right:0;height:${boxH}%`}
				></div>
				<div class="scrim" style={`left:0;top:${Y + boxH}%;right:0;bottom:0`}></div>
			</div>

			<!-- Selection marquee -->
			<div
				class={cn(
					"absolute border",
					phase === "recording" ? "border-destructive" : "border-primary",
				)}
				style={`left:${X}%;top:${Y}%;width:${boxW}%;height:${boxH}%`}
			>
				{#each ["-top-1 -left-1", "-top-1 -right-1", "-bottom-1 -left-1", "-bottom-1 -right-1"] as pos (pos)}
					<span
						aria-hidden="true"
						class={cn(
							"absolute size-1.5 rounded-[2px] border bg-card",
							phase === "recording" ? "border-destructive" : "border-primary",
							pos,
						)}
					></span>
				{/each}

				<!-- Size readout, pinned inside the box's top-left. -->
				<span
					class="absolute left-1.5 top-1.5 rounded bg-foreground px-1.5 py-0.5 text-caption font-medium text-background tabular-nums"
				>
					{px.w} × {px.h}
				</span>
			</div>
		{/if}

		<!-- Capture bar. Rises once the selection settles. -->
		<div
			class={cn(
				"absolute bottom-2.5 left-1/2 flex -translate-x-1/2 items-center gap-1.5 rounded-full border border-border-low bg-card px-1.5 py-1 shadow-craft-sm transition-all duration-300 ease-[cubic-bezier(0.625,0.05,0,1)] motion-reduce:transition-none",
				phase === "armed" || phase === "recording"
					? "translate-y-0 opacity-100"
					: "pointer-events-none translate-y-3 opacity-0",
			)}
		>
			<span
				class={cn(
					"inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 text-caption font-medium",
					phase === "recording"
						? "bg-destructive text-destructive-foreground"
						: "bg-foreground text-background",
				)}
			>
				{#if phase === "recording"}
					<span class="size-1.5 rounded-full bg-current"></span>
					{elapsedLabel}
				{:else}
					<Video class="size-3" />
					Record
				{/if}
			</span>
			<span class="h-3.5 w-px bg-border-low"></span>
			<Mic class="size-3 text-muted-foreground" />
			<Camera class="size-3 text-muted-foreground" />
		</div>
	</div>
</div>

<style>
	/* Dimming must darken in both themes: a foreground-tinted scrim brightens
	   the excluded area in dark mode. */
	.scrim {
		position: absolute;
		background-color: oklch(0% 0 0 / 0.1);
	}

	:global(.dark) .scrim,
	:global([data-theme="dark"]) .scrim {
		background-color: oklch(0% 0 0 / 0.5);
	}
</style>
