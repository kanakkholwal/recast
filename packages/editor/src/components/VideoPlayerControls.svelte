<script lang="ts">
import type { EditorStore } from "$lib/stores/editor-store.svelte";
import { originalToOutput, outputToOriginal } from "$lib/timeline/time-map";
import { formatTimeByMode, frameStepOutput } from "$lib/editor/time";
import {
	Camera,
	LoaderCircle,
	Maximize2,
	Minimize2,
	Pause,
	Play,
	Repeat,
	SkipBack,
	SkipForward,
} from "@recast/icons";
import { Kbd } from "@recast/ui/kbd";
import { toast } from "@recast/ui/sonner";
import * as Tooltip from "@recast/ui/tooltip";
import { cn } from "@recast/ui/utils";
import { onDestroy } from "svelte";
import MarkupControls from "./_components/MarkupControls.svelte";
import { BAR_BTN, BAR_BTN_DISABLED, BAR_BTN_ON, BAR_GROUP } from "./_components/player-bar.styles";

interface Props {
	store: EditorStore;
	videoEl?: HTMLVideoElement | null;
	/** Element to request fullscreen on (usually the preview container). */
	fullscreenTargetEl?: HTMLElement | null;
	/** PNG blob of the current preview composite; undefined disables Copy-frame (WebGL2 not ready). */
	captureFrame?: (() => Promise<Blob | null>) | undefined;
	/** Loop toggle. Just flips the flag here; the editor page does the seek-and-replay (needs audio + `ended`). */
	loopEnabled?: boolean;
	/** Whether the timeline is hiding, so this bar owns the scrubbing. Off when
	 *  the timeline is visible (it is the better scrubber): two scrubbers for one
	 *  position is redundant, and only the timeline shows cuts/zoom/markup.
	 *  Fullscreen overrides this — see `scrubberVisible`. */
	showScrubber?: boolean;
}

let {
	store,
	videoEl = null,
	fullscreenTargetEl = null,
	captureFrame = undefined,
	loopEnabled = $bindable(false),
	showScrubber = true,
}: Props = $props();

let capturing = $state(false);

// Copy the current frame to the clipboard as PNG. `navigator.clipboard.write`
// works on Tauri (tauri://localhost is a secure context). Pause first so the captured pixels match.
async function copyFrameToClipboard() {
	if (capturing || !captureFrame) return;
	capturing = true;
	const wasPlaying = store.isPlaying;
	if (wasPlaying && videoEl) {
		videoEl.pause();
		store.isPlaying = false;
	}
	try {
		const blob = await captureFrame();
		if (!blob) {
			toast.error("Couldn't capture frame. Preview isn't ready yet.");
			return;
		}
		await navigator.clipboard.write([new ClipboardItem({ "image/png": blob })]);
		toast.success("Frame copied to clipboard.");
	} catch (err) {
		toast.error(`Couldn't copy frame: ${(err as Error)?.message ?? String(err)}`);
	} finally {
		capturing = false;
	}
}

let isFullscreen = $state(false);

// Mirror the browser's fullscreen state so the toggle icon reflects reality.
$effect(() => {
	const handler = () => {
		isFullscreen = !!document.fullscreenElement;
	};
	document.addEventListener("fullscreenchange", handler);
	return () => document.removeEventListener("fullscreenchange", handler);
});

async function toggleFullscreen() {
	if (document.fullscreenElement) {
		await document.exitFullscreen();
		return;
	}
	if (fullscreenTargetEl) await fullscreenTargetEl.requestFullscreen();
}

// Fullscreen puts ONLY the preview container on screen, so the timeline isn't
// there to scrub with however `showScrubber` was resolved — leaving fullscreen
// with no transport at all whenever the timeline happened to be open.
const scrubberVisible = $derived(showScrubber || isFullscreen);

// OUTPUT (post-cut) time: readout/scrubber reflect the edited length and can't land
// in a removed region. `store.currentTime` stays the source of truth (original time); we map to output only for display + seek.
const timeMap = $derived(store.timeMap);
const fullDuration = $derived(store.metadata?.duration ?? 0);
const outputDuration = $derived(originalToOutput(timeMap, fullDuration));
const currentOutput = $derived(originalToOutput(timeMap, store.currentTime));

// Output time under the pointer while scrubbing, or null when the store owns the
// position. The thumb used to be driven by `store.currentTime`, so it fought the
// drag whenever a seek landed late.
let scrubOutput = $state<number | null>(null);
const displayOutput = $derived(scrubOutput ?? currentOutput);

// Same formatter and same Time-display setting as the timeline, so the readout
// and the playhead can't show two different numbers for one moment.
const fps = $derived(store.metadata?.fps || 60);
const currentTimeFormatted = $derived(formatTimeByMode(displayOutput, store.timeMode, fps));
const durationFormatted = $derived(formatTimeByMode(outputDuration, store.timeMode, fps));
/** Thumb diameter. The fill and the thumb are placed from this same number, so
 *  it has to match the rendered size (`size-3`). */
const THUMB_PX = 12;

const progressFraction = $derived(
	outputDuration > 0 ? Math.min(1, Math.max(0, displayOutput / outputDuration)) : 0,
);

// A range input's thumb travels between its own half-widths, not 0→100%, so a
// fill sized at a flat `fraction * 100%` only lines up with the thumb at the two
// ends and drifts by up to a quarter thumb in between. Both the fill's width and
// the thumb's centre are placed with THIS expression, so they cannot disagree at
// any position, and it needs no measurement of the track.
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

// Every seek here goes through `store.seek`, which moves the playhead AND the
// registered transport. Writing `videoEl.currentTime` directly left the system
// and microphone audio elements behind, so a frame-step or a scrub desynced
// sound from picture on the <video> path.
function stepFrame(direction: number) {
	if (!store.metadata) return;
	// Step on the OUTPUT axis so stepping past a cut boundary lands on the next
	// kept frame instead of inside the removed range.
	store.seek(frameStepOutput(timeMap, store.metadata, store.currentTime, direction));
}

// The scrubber is in output time; map back to original (skipping over collapsed
// cuts) before driving the transport.
function seekToOutput(outputTime: number) {
	store.seek(outputToOriginal(timeMap, outputTime));
}

// Coalesced to one seek per frame. `oninput` fires per pointer-pixel and each
// seek now moves three media elements, so writing straight through thrashed the
// decoder for positions no one ever saw.
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

		<!-- Centre: drawing tools, directly under the picture they draw on. Empty on
		     every tab but Markup, so the flanks keep their positions regardless. -->
		<div class="flex shrink-0 items-center gap-2">
			<MarkupControls {store} />
		</div>

		<div class="flex min-w-0 flex-1 shrink-0 items-center justify-end">
			<div class={BAR_GROUP}>
				<Tooltip.Root>
					<Tooltip.Trigger>
						<!-- A native `disabled` button swallows pointer events, so the two
						     tooltips that explain WHY it's disabled never fired. The span
						     carries the trigger; the button inside stays properly disabled. -->
						{#snippet child({ props })}
							<span {...props as Record<string, unknown>} class="inline-flex">
								<button
									type="button"
									onclick={copyFrameToClipboard}
									disabled={!captureFrame || capturing}
									aria-label="Copy current frame to clipboard"
									class={cn(BAR_BTN, BAR_BTN_DISABLED)}
								>
									{#if capturing}
										<LoaderCircle size={13} class="animate-spin" />
									{:else}
										<Camera size={13} />
									{/if}
								</button>
							</span>
						{/snippet}
					</Tooltip.Trigger>
					<Tooltip.Content>
						{#if capturing}
							Copying frame…
						{:else if !captureFrame}
							Preview isn't ready yet
						{:else}
							Copy frame to clipboard
						{/if}
					</Tooltip.Content>
				</Tooltip.Root>

				<Tooltip.Root>
					<Tooltip.Trigger>
						{#snippet child({ props })}
							<button
								{...props}
								type="button"
								onclick={() => (loopEnabled = !loopEnabled)}
								aria-pressed={loopEnabled}
								aria-label="Loop within the trim"
								class={cn(BAR_BTN, loopEnabled && [BAR_BTN_ON, "text-primary"])}
							>
								<Repeat size={13} />
							</button>
						{/snippet}
					</Tooltip.Trigger>
					<Tooltip.Content>
						{loopEnabled ? "Looping within the trim" : "Loop within the trim"}
					</Tooltip.Content>
				</Tooltip.Root>

				<Tooltip.Root>
					<Tooltip.Trigger>
						<!-- Same disabled-swallows-pointer-events wrapper as Copy frame.
						     The label flips with state, so no aria-pressed on top of it:
						     "Exit fullscreen, pressed" is one signal too many. -->
						{#snippet child({ props })}
							<span {...props as Record<string, unknown>} class="inline-flex">
								<button
									type="button"
									onclick={toggleFullscreen}
									disabled={!fullscreenTargetEl}
									aria-label={isFullscreen ? "Exit fullscreen" : "Enter fullscreen"}
									class={cn(BAR_BTN, BAR_BTN_DISABLED)}
								>
									{#if isFullscreen}
										<Minimize2 size={13} />
									{:else}
										<Maximize2 size={13} />
									{/if}
								</button>
							</span>
						{/snippet}
					</Tooltip.Trigger>
					<Tooltip.Content>
						<span class="inline-flex items-center gap-1.5">
							{isFullscreen ? "Exit fullscreen" : "Fullscreen"} <Kbd>F</Kbd>
						</span>
					</Tooltip.Content>
				</Tooltip.Root>
			</div>
		</div>
	</div>
</div>
