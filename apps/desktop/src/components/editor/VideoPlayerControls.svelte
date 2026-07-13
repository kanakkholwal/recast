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
	} from "@lucide/svelte";
	import { Kbd } from "@recast/ui/kbd";
	import { toast } from "@recast/ui/sonner";
	import * as Tooltip from "@recast/ui/tooltip";
	import { cn } from "@recast/ui/utils";

	interface Props {
		store: EditorStore;
		videoEl?: HTMLVideoElement | null;
		/** Element to request fullscreen on (usually the preview container). */
		fullscreenTargetEl?: HTMLElement | null;
		/** PNG blob of the current preview composite; undefined disables Copy-frame (WebGL2 not ready). */
		captureFrame?: (() => Promise<Blob | null>) | undefined;
		/** Loop toggle. Just flips the flag here; the editor page does the seek-and-replay (needs audio + `ended`). */
		loopEnabled?: boolean;
	}

	let {
		store,
		videoEl = null,
		fullscreenTargetEl = null,
		captureFrame = undefined,
		loopEnabled = $bindable(false),
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
			await navigator.clipboard.write([
				new ClipboardItem({ "image/png": blob }),
			]);
			toast.success("Frame copied to clipboard.");
		} catch (err) {
			toast.error(
				`Couldn't copy frame: ${(err as Error)?.message ?? String(err)}`,
			);
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

	// OUTPUT (post-cut) time: readout/scrubber reflect the edited length and can't land
	// in a removed region. `store.currentTime` stays the source of truth (original time); we map to output only for display + seek.
	const timeMap = $derived(store.timeMap);
	const fullDuration = $derived(store.metadata?.duration ?? 0);
	const outputDuration = $derived(originalToOutput(timeMap, fullDuration));
	const currentOutput = $derived(originalToOutput(timeMap, store.currentTime));

	// Same formatter and same Time-display setting as the timeline, so the readout
	// and the playhead can't show two different numbers for one moment.
	const fps = $derived(store.metadata?.fps || 60);
	const currentTimeFormatted = $derived(
		formatTimeByMode(currentOutput, store.timeMode, fps),
	);
	const durationFormatted = $derived(
		formatTimeByMode(outputDuration, store.timeMode, fps),
	);
	const progressPct = $derived(
		outputDuration > 0
			? Math.min(100, (currentOutput / outputDuration) * 100)
			: 0,
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

	function stepFrame(direction: number) {
		if (!store.metadata) return;
		// Step on the OUTPUT axis so stepping past a cut boundary lands on the next
		// kept frame instead of inside the removed range.
		const orig = frameStepOutput(
			timeMap,
			store.metadata,
			store.currentTime,
			direction,
		);
		if (videoEl) videoEl.currentTime = orig;
		store.currentTime = orig;
	}

	function handleSeek(e: Event) {
		const target = e.target as HTMLInputElement;
		// The scrubber is in output time; map back to original time (skipping over
		// collapsed cuts) before driving the transport.
		const orig = outputToOriginal(timeMap, parseFloat(target.value));
		if (videoEl) videoEl.currentTime = orig;
		store.currentTime = orig;
	}
</script>

<div class="flex h-10 w-full items-center gap-2 px-2">
	<div
		class="flex items-center gap-0.5 rounded-lg bg-muted/60 p-0.5 ring-1 ring-inset ring-border/40"
	>
		<Tooltip.Root>
			<Tooltip.Trigger>
				{#snippet child({ props })}
					<button
						{...props}
						type="button"
						onclick={() => stepFrame(-1)}
						aria-label="Previous frame"
						class="cursor-pointer flex size-6 items-center justify-center rounded-md text-muted-foreground transition-colors duration-150 hover:bg-card hover:text-foreground"
					>
						<SkipBack size={12} />
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
						class="cursor-pointer flex size-7 items-center justify-center rounded-md bg-card text-foreground shadow-(--shadow-craft-inset) ring-1 ring-inset ring-border/40 transition-transform duration-150 hover:scale-105 active:scale-95"
					>
						{#if store.isPlaying}
							<Pause size={12} fill="currentColor" />
						{:else}
							<Play size={12} fill="currentColor" />
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
						class="cursor-pointer flex size-6 items-center justify-center rounded-md text-muted-foreground transition-colors duration-150 hover:bg-card hover:text-foreground"
					>
						<SkipForward size={12} />
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

	<div
		class="flex items-center gap-1 font-mono tabular-nums text-[11px] font-semibold min-w-32"
	>
		<span class="text-foreground">{currentTimeFormatted}</span>
		<span class="text-muted-foreground/40">/</span>
		<span class="text-muted-foreground">{durationFormatted}</span>
	</div>

	<div class="relative flex h-7 flex-1 items-center">
		<div
			class="pointer-events-none absolute inset-x-0 top-1/2 h-1 -translate-y-1/2 rounded-full bg-muted/80 ring-1 ring-inset ring-border/40"
			aria-hidden="true"
		></div>
		<div
			class="pointer-events-none absolute top-1/2 left-0 h-1 -translate-y-1/2 rounded-full bg-primary"
			style="width: {progressPct}%"
			aria-hidden="true"
		></div>
		<!-- Step is one frame, not 10ms: arrow keys here now move exactly as far as
		     the frame-step buttons whose tooltips advertise the same arrows. -->
		<input
			type="range"
			min="0"
			max={outputDuration}
			step={1 / fps}
			value={currentOutput}
			oninput={handleSeek}
			class="relative z-10 m-0 h-3 w-full cursor-pointer appearance-none bg-transparent p-0 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring [&::-webkit-slider-runnable-track]:h-3 [&::-webkit-slider-runnable-track]:bg-transparent [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:size-3 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-primary [&::-webkit-slider-thumb]:shadow-(--shadow-craft-inset) [&::-webkit-slider-thumb]:ring-2 [&::-webkit-slider-thumb]:ring-background [&::-webkit-slider-thumb]:transition-transform hover:[&::-webkit-slider-thumb]:scale-125 active:[&::-webkit-slider-thumb]:scale-110"
			aria-label="Video progress"
			aria-valuetext={currentTimeFormatted}
		/>
	</div>

	<div
		class="flex items-center gap-0.5 rounded-lg bg-muted/60 p-0.5 ring-1 ring-inset ring-border/40"
	>
		<Tooltip.Root>
			<Tooltip.Trigger>
				{#snippet child({ props })}
					<button
						{...props}
						type="button"
						onclick={copyFrameToClipboard}
						disabled={!captureFrame || capturing}
						aria-label="Copy current frame to clipboard"
						class="cursor-pointer flex size-6 items-center justify-center rounded-md text-muted-foreground transition-colors duration-150 hover:bg-card hover:text-foreground disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent disabled:hover:text-muted-foreground"
					>
						{#if capturing}
							<LoaderCircle size={12} class="animate-spin" />
						{:else}
							<Camera size={12} />
						{/if}
					</button>
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
						aria-label="Loop within trim"
						class={cn(
							"flex size-6 items-center justify-center rounded-md transition-colors duration-150",
							loopEnabled
								? "bg-card text-primary shadow-(--shadow-craft-inset) ring-1 ring-inset ring-border/40"
								: "text-muted-foreground hover:bg-card hover:text-foreground",
						)}
					>
						<Repeat size={12} />
					</button>
				{/snippet}
			</Tooltip.Trigger>
			<Tooltip.Content>
				{loopEnabled ? "Looping within the trim" : "Loop within the trim"}
			</Tooltip.Content>
		</Tooltip.Root>

		<Tooltip.Root>
			<Tooltip.Trigger>
				{#snippet child({ props })}
					<button
						{...props}
						type="button"
						onclick={toggleFullscreen}
						disabled={!fullscreenTargetEl}
						aria-pressed={isFullscreen}
						aria-label={isFullscreen ? "Exit fullscreen" : "Enter fullscreen"}
						class="flex size-6 items-center justify-center rounded-md text-muted-foreground transition-colors duration-150 hover:bg-card hover:text-foreground disabled:opacity-40"
					>
						{#if isFullscreen}
							<Minimize2 size={12} />
						{:else}
							<Maximize2 size={12} />
						{/if}
					</button>
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
