<script lang="ts">
import { Pause, Play, SkipBack, SkipForward } from "@recast/icons";
import { Kbd } from "@recast/ui/kbd";
import * as Tooltip from "@recast/ui/tooltip";
import { cn } from "@recast/ui/utils";
import { onDestroy } from "svelte";
import { formatTimeByMode, frameStepOutput } from "../lib/editor/time";
import { originalToOutput, outputToOriginal } from "../lib/timeline/time-map";
import type { EditorStore } from "../stores/editor-store.svelte";
import { BAR_BTN, BAR_BTN_ON, BAR_GROUP } from "./_components/player-bar.styles";

interface Props {
	store: EditorStore;
	videoEl?: HTMLVideoElement | null;
	/** Whether the timeline is hiding, so this bar owns the scrubbing. Off when
	 *  the timeline is visible (it is the better scrubber): two scrubbers for one
	 *  position is redundant, and only the timeline shows cuts/zoom/markup.
	 *  Fullscreen overrides this — see `scrubberVisible`. */
	showScrubber?: boolean;
}

let { store, videoEl = null, showScrubber = true }: Props = $props();

let isFullscreen = $state(false);

// Mirror the browser's fullscreen state; the scrubber follows it (fullscreen has no timeline).
$effect(() => {
	const handler = () => {
		isFullscreen = Boolean(document.fullscreenElement);
	};
	document.addEventListener("fullscreenchange", handler);
	return () => document.removeEventListener("fullscreenchange", handler);
});

// Fullscreen shows only the preview, so without this, leaving it with the timeline open left no transport at all.
const scrubberVisible = $derived(showScrubber || isFullscreen);

// OUTPUT time for display and seek only; `store.currentTime` stays the source of truth in original time.
const timeMap = $derived(store.timeMap);
const fullDuration = $derived(store.metadata?.duration ?? 0);
const outputDuration = $derived(originalToOutput(timeMap, fullDuration));
const currentOutput = $derived(originalToOutput(timeMap, store.currentTime));

// Null when the store owns the position: driving the thumb from `store.currentTime` fought the drag on a late seek.
let scrubOutput = $state<number | null>(null);
const displayOutput = $derived(scrubOutput ?? currentOutput);

// Same formatter and Time-display setting as the timeline, so the readout and the playhead can't disagree.
const fps = $derived(store.metadata?.fps || 60);
const currentTimeFormatted = $derived(formatTimeByMode(displayOutput, store.timeMode, fps));
const durationFormatted = $derived(formatTimeByMode(outputDuration, store.timeMode, fps));
/** Thumb diameter. The fill and the thumb are placed from this same number, so
 *  it has to match the rendered size (`size-3`). */
const THUMB_PX = 12;

const progressFraction = $derived(
	outputDuration > 0 ? Math.min(1, Math.max(0, displayOutput / outputDuration)) : 0,
);

// A range thumb travels between its own half-widths, so a flat percentage fill drifts; both fill and thumb use this expression.
const progressOffset = $derived(
	`calc(${progressFraction} * (100% - ${THUMB_PX}px) + ${THUMB_PX / 2}px)`,
);

function togglePlay() {
	if (!videoEl) return;
	if (store.isPlaying) {
		videoEl.pause();
		store.isPlaying = false;
	} else {
		void videoEl.play();
		store.isPlaying = true;
	}
}

// `store.seek` moves the playhead AND the transport; writing `videoEl.currentTime` left the audio elements behind.
function stepFrame(direction: number) {
	if (!store.metadata) return;
	// Step on the OUTPUT axis so stepping past a cut lands on the next kept frame, not inside the removed range.
	store.seek(frameStepOutput(timeMap, store.metadata, store.currentTime, direction));
}

// The scrubber is output time; map back to original, skipping collapsed cuts, before driving the transport.
function seekToOutput(outputTime: number) {
	store.seek(outputToOriginal(timeMap, outputTime));
}

// One seek per frame: `oninput` fires per pointer-pixel, and each seek moves three media elements.
let scrubRaf: number | null = null;

function flushScrub() {
	scrubRaf = null;
	if (scrubOutput !== null) seekToOutput(scrubOutput);
}

function handleScrubInput(e: Event) {
	scrubOutput = parseFloat((e.target as HTMLInputElement).value);
	if (scrubRaf === null) scrubRaf = requestAnimationFrame(flushScrub);
}

/** Pointer release (and every keyboard step): land exactly on the committed
 *  value, then hand the readout back to the store. */
function handleScrubCommit(e: Event) {
	if (scrubRaf !== null) {
		cancelAnimationFrame(scrubRaf);
		scrubRaf = null;
	}
	seekToOutput(parseFloat((e.target as HTMLInputElement).value));
	scrubOutput = null;
}

onDestroy(() => {
	if (scrubRaf !== null) cancelAnimationFrame(scrubRaf);
});
</script>

<!-- Transport left, markup centred, view controls right. Capped at the preview's
     own max width so the outer groups sit under the picture rather than out at
     the column edges, where they read as belonging to nothing. -->
<div class="mx-auto flex w-full max-w-280 flex-col gap-1 px-2 py-1">
	{#if scrubberVisible}
		<!-- Full width, on its own row. Squeezed between the readout and the right
		     cluster it had a few hundred pixels for the whole recording, which made
		     precise scrubbing impossible. Shown when the timeline is collapsed (it
		     is the better scrubber) and always in fullscreen. -->
		<div class="group/scrub relative flex h-4 w-full items-center">
			<div
				class="pointer-events-none absolute inset-x-0 top-1/2 h-1 -translate-y-1/2 rounded-full bg-muted/80 ring-1 ring-inset ring-border/40"
				aria-hidden="true"
			></div>
			<!-- Fill and thumb are DRAWN BY US from one offset, and the input's own
			     thumb is transparent. Styling the native thumb meant the fill (sized
			     as a plain percentage) and the thumb (inset by its own half-width)
			     tracked two different geometries; no tween can reconcile that, and a
			     transition on only one of the two just made the fill trail the thumb. -->
			<div
				class="pointer-events-none absolute top-1/2 left-0 h-1 -translate-y-1/2 rounded-full bg-primary"
				style="width: {progressOffset};"
				aria-hidden="true"
			></div>
			<div
				class="pointer-events-none absolute top-1/2 z-10 size-3 -translate-x-1/2 -translate-y-1/2 rounded-full bg-primary shadow-(--shadow-craft-inset) ring-2 ring-background transition-transform group-hover/scrub:scale-125 group-active/scrub:scale-110"
				style="left: {progressOffset};"
				aria-hidden="true"
			></div>
			<!-- Interaction and a11y only; the visuals above ride on the same value.
			     Step is one frame, not 10ms: arrow keys here move exactly as far as
			     the frame-step buttons whose tooltips advertise the same arrows. -->
			<input
				type="range"
				min="0"
				max={outputDuration}
				step={1 / fps}
				value={displayOutput}
				oninput={handleScrubInput}
				onchange={handleScrubCommit}
				class="relative z-20 m-0 h-3 w-full cursor-pointer appearance-none bg-transparent p-0 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring [&::-webkit-slider-runnable-track]:h-3 [&::-webkit-slider-runnable-track]:bg-transparent [&::-webkit-slider-thumb]:size-3 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-transparent"
				aria-label="Video progress"
				aria-valuetext={currentTimeFormatted}
			/>
		</div>
	{/if}

	<!-- Equal-weight outer zones rather than a bare `justify-between`: with three
	     children of unequal width, space-between only centres the middle one by
	     coincidence. flex-1 on both flanks keeps markup truly centred. -->
	<div class="flex h-8 w-full items-center justify-between gap-2">
		<!-- Transport + timecode live in the timeline header now; shown here only
		     when the timeline is hidden or in fullscreen, where nothing else has them. -->
		{#if scrubberVisible}
		<div class="flex min-w-0 flex-1 shrink-0 items-center gap-2">
			<div class={BAR_GROUP}>
				<Tooltip.Root>
					<Tooltip.Trigger>
						{#snippet child({ props })}
							<button
								{...props}
								type="button"
								onclick={() => stepFrame(-1)}
								aria-label="Previous frame"
								class={BAR_BTN}
							>
								<SkipBack size={13} />
							</button>
						{/snippet}
					</Tooltip.Trigger>
					<Tooltip.Content>
						<span class="inline-flex items-center gap-1.5">
							Previous frame <Kbd>←</Kbd>
						</span>
					</Tooltip.Content>
				</Tooltip.Root>

				<Tooltip.Root>
					<Tooltip.Trigger>
						{#snippet child({ props })}
							<button
								{...props}
								type="button"
								onclick={togglePlay}
								aria-label={store.isPlaying ? "Pause" : "Play"}
								class={cn(
									BAR_BTN,
									BAR_BTN_ON,
									"hover:scale-105 active:scale-95 motion-reduce:hover:scale-100",
								)}
							>
								{#if store.isPlaying}
									<Pause size={13} fill="currentColor" />
								{:else}
									<Play size={13} fill="currentColor" />
								{/if}
							</button>
						{/snippet}
					</Tooltip.Trigger>
					<Tooltip.Content>
						<span class="inline-flex items-center gap-1.5">
							{store.isPlaying ? "Pause" : "Play"} <Kbd>Space</Kbd>
						</span>
					</Tooltip.Content>
				</Tooltip.Root>

				<Tooltip.Root>
					<Tooltip.Trigger>
						{#snippet child({ props })}
							<button
								{...props}
								type="button"
								onclick={() => stepFrame(1)}
								aria-label="Next frame"
								class={BAR_BTN}
							>
								<SkipForward size={13} />
							</button>
						{/snippet}
					</Tooltip.Trigger>
					<Tooltip.Content>
						<span class="inline-flex items-center gap-1.5">
							Next frame <Kbd>→</Kbd>
						</span>
					</Tooltip.Content>
				</Tooltip.Root>
			</div>

			<!-- Fixed width so nothing downstream shifts as the digits change. -->
			<div class="flex w-28 items-center gap-1 font-mono text-[11px] font-semibold tabular-nums">
				<span class="text-foreground">{currentTimeFormatted}</span>
				<span class="text-muted-foreground/40">/</span>
				<span class="text-muted-foreground">{durationFormatted}</span>
			</div>
		</div>
		{:else}
			<div class="flex-1"></div>
		{/if}
	</div>
</div>
