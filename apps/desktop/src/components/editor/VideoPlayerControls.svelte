<script lang="ts">
	import type { EditorStore } from "$lib/stores/editor-store.svelte";
	import {
	  Maximize2,
	  Minimize2,
	  Pause,
	  Play,
	  Repeat,
	  SkipBack,
	  SkipForward,
	} from "@lucide/svelte";
	import { Kbd } from "@recast/ui/kbd";
	import * as Tooltip from "@recast/ui/tooltip";
	import { cn } from "@recast/ui/utils";

	interface Props {
		store: EditorStore;
		videoEl?: HTMLVideoElement | null;
		/** Element to request fullscreen on (usually the preview container). */
		fullscreenTargetEl?: HTMLElement | null;
	}

	let { store, videoEl = null, fullscreenTargetEl = null }: Props = $props();

	let loopEnabled = $state(false);
	let isFullscreen = $state(false);

	// Loop: at trimEnd, seek back to trimStart instead of letting the clip end.
	$effect(() => {
		if (!videoEl || !loopEnabled) return;
		const onTime = () => {
			if (!videoEl) return;
			const effectiveEnd =
				store.trimEnd || store.metadata?.duration || Infinity;
			if (videoEl.currentTime >= effectiveEnd - 0.04) {
				videoEl.currentTime = store.trimStart;
				if (!store.isPlaying) {
					void videoEl.play();
					store.isPlaying = true;
				}
			}
		};
		videoEl.addEventListener("timeupdate", onTime);
		return () => videoEl?.removeEventListener("timeupdate", onTime);
	});

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

	function formatTime(seconds: number): string {
		const mins = Math.floor(seconds / 60);
		const secs = Math.floor(seconds % 60);
		const ms = Math.floor((seconds % 1) * 100);
		return `${mins}:${secs.toString().padStart(2, "0")}.${ms.toString().padStart(2, "0")}`;
	}

	const currentTimeFormatted = $derived(formatTime(store.currentTime));
	const durationFormatted = $derived(
		formatTime(store.metadata?.duration ?? 0),
	);
	const duration = $derived(store.metadata?.duration ?? 0);
	const progressPct = $derived(
		duration > 0 ? Math.min(100, (store.currentTime / duration) * 100) : 0,
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
		if (!videoEl || !store.metadata) return;
		const frameDuration = 1 / (store.metadata.fps || 30);
		videoEl.currentTime = Math.max(
			0,
			Math.min(
				videoEl.currentTime + frameDuration * direction,
				store.metadata.duration,
			),
		);
		store.currentTime = videoEl.currentTime;
	}

	function handleSeek(e: Event) {
		const target = e.target as HTMLInputElement;
		const val = parseFloat(target.value);
		if (videoEl) videoEl.currentTime = val;
		store.currentTime = val;
	}
</script>

<div class="flex h-10 w-full items-center gap-2 px-2">
	<!-- Transport pill: play / step -->
	<div
		class="flex items-center gap-0.5 rounded-lg bg-muted/60 p-0.5 ring-1 ring-inset ring-border/40"
	>
		<Tooltip.Root>
			<Tooltip.Trigger>
				<button
					type="button"
					onclick={() => stepFrame(-1)}
					aria-label="Previous frame"
					class="cursor-pointer flex size-6 items-center justify-center rounded-md text-muted-foreground transition-colors duration-150 hover:bg-card hover:text-foreground"
				>
					<SkipBack size={12} />
				</button>
			</Tooltip.Trigger>
			<Tooltip.Content>
				<span class="inline-flex items-center gap-1.5">
					Previous frame <Kbd>←</Kbd>
				</span>
			</Tooltip.Content>
		</Tooltip.Root>

		<Tooltip.Root>
			<Tooltip.Trigger>
				<button
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
			</Tooltip.Trigger>
			<Tooltip.Content>
				<span class="inline-flex items-center gap-1.5">
					{store.isPlaying ? "Pause" : "Play"} <Kbd>Space</Kbd>
				</span>
			</Tooltip.Content>
		</Tooltip.Root>

		<Tooltip.Root>
			<Tooltip.Trigger>
				<button
					type="button"
					onclick={() => stepFrame(1)}
					aria-label="Next frame"
					class="cursor-pointer flex size-6 items-center justify-center rounded-md text-muted-foreground transition-colors duration-150 hover:bg-card hover:text-foreground"
				>
					<SkipForward size={12} />
				</button>
			</Tooltip.Trigger>
			<Tooltip.Content>
				<span class="inline-flex items-center gap-1.5">
					Next frame <Kbd>→</Kbd>
				</span>
			</Tooltip.Content>
		</Tooltip.Root>
	</div>

	<!-- Time readout -->
	<div
		class="flex items-center gap-1 font-mono tabular-nums text-[11px] font-semibold min-w-32"
	>
		<span class="text-foreground">{currentTimeFormatted}</span>
		<span class="text-muted-foreground/40">/</span>
		<span class="text-muted-foreground">{durationFormatted}</span>
	</div>

	<!-- Scrubber: custom thin track with played overlay -->
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
		<input
			type="range"
			min="0"
			max={duration}
			step="0.01"
			value={store.currentTime}
			oninput={handleSeek}
			class="relative z-10 m-0 h-3 w-full cursor-pointer appearance-none bg-transparent p-0 focus:outline-none [&::-webkit-slider-runnable-track]:h-3 [&::-webkit-slider-runnable-track]:bg-transparent [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:size-3 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-primary [&::-webkit-slider-thumb]:shadow-(--shadow-craft-inset) [&::-webkit-slider-thumb]:ring-2 [&::-webkit-slider-thumb]:ring-background [&::-webkit-slider-thumb]:transition-transform hover:[&::-webkit-slider-thumb]:scale-125 active:[&::-webkit-slider-thumb]:scale-110"
			aria-label="Video progress"
		/>
	</div>

	<!-- Right: loop + fullscreen -->
	<div
		class="flex items-center gap-0.5 rounded-lg bg-muted/60 p-0.5 ring-1 ring-inset ring-border/40"
	>
		<Tooltip.Root>
			<Tooltip.Trigger>
				<button
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
			</Tooltip.Trigger>
			<Tooltip.Content
				>{loopEnabled ? "Loop on" : "Loop off"}</Tooltip.Content
			>
		</Tooltip.Root>

		<Tooltip.Root>
			<Tooltip.Trigger>
				<button
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
			</Tooltip.Trigger>
			<Tooltip.Content>
				<span class="inline-flex items-center gap-1.5">
					{isFullscreen ? "Exit fullscreen" : "Fullscreen"} <Kbd>F</Kbd>
				</span>
			</Tooltip.Content>
		</Tooltip.Root>
	</div>
</div>
