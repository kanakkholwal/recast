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
	import { Button } from "@recast/ui/button";
	import { Input } from "@recast/ui/input";
	import * as Tooltip from "@recast/ui/tooltip";

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

	// Mirror the browser's fullscreen state so the toggle icon reflects reality
	// (user pressing Esc, etc.).
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

<div
	class="flex h-11 w-full items-center justify-between gap-3 px-2"
>
	<div class="flex items-center gap-0.5 text-foreground">
		<Tooltip.Root>
			<Tooltip.Trigger>
				<Button
					variant="ghost"
					size="icon-sm"
					onclick={togglePlay}
					aria-label={store.isPlaying ? "Pause" : "Play"}
					title={store.isPlaying ? "Pause (Space)" : "Play (Space)"}
				>
					{#if store.isPlaying}
						<Pause size={12} fill="currentColor" />
					{:else}
						<Play size={12} fill="currentColor" />
					{/if}
				</Button>
			</Tooltip.Trigger>
			<Tooltip.Content
				>{store.isPlaying ? "Pause" : "Play"} (Space)</Tooltip.Content
			>
		</Tooltip.Root>

		<Tooltip.Root>
			<Tooltip.Trigger>
				<Button
					variant="ghost"
					size="icon-sm"
					onclick={() => stepFrame(-1)}
					aria-label="Previous frame"
					title="Previous frame (←)"
				>
					<SkipBack size={12} />
				</Button>
			</Tooltip.Trigger>
			<Tooltip.Content>Previous frame (←)</Tooltip.Content>
		</Tooltip.Root>

		<Tooltip.Root>
			<Tooltip.Trigger>
				<Button
					variant="ghost"
					size="icon-sm"
					onclick={() => stepFrame(1)}
					aria-label="Next frame"
					title="Next frame (→)"
				>
					<SkipForward size={12} />
				</Button>
			</Tooltip.Trigger>
			<Tooltip.Content>Next frame (→)</Tooltip.Content>
		</Tooltip.Root>
	</div>

	<div
		class="flex items-center gap-1.5 font-mono tabular-nums text-[11px] font-medium text-muted-foreground min-w-20 text-center"
	>
		<span class="text-foreground">{currentTimeFormatted}</span>
		<span class="text-muted-foreground/40">/</span>
		<span>{durationFormatted}</span>
	</div>

	<!-- Scrubber -->
	<div class="flex-1">
		<Input
			type="range"
			min="0"
			max={store.metadata?.duration ?? 0}
			step="0.01"
			value={store.currentTime}
			oninput={handleSeek}
			class="h-1 w-full cursor-pointer appearance-none rounded-full bg-border focus:outline-none focus:ring-2 focus:ring-primary/50 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:size-3 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-primary [&::-webkit-slider-thumb]:shadow-sm hover:[&::-webkit-slider-thumb]:scale-110 active:[&::-webkit-slider-thumb]:scale-95 [&::-webkit-slider-thumb]:transition-transform"
			aria-label="Video progress"
		/>
	</div>

	<div class="flex items-center gap-0.5 text-foreground">
		<Tooltip.Root>
			<Tooltip.Trigger>
				<Button
					variant={loopEnabled ? "default_soft" : "ghost"}
					size="icon-sm"
					onclick={() => (loopEnabled = !loopEnabled)}
					aria-pressed={loopEnabled}
					aria-label="Loop playback within trim"
					title="Loop playback within trim"
				>
					<Repeat size={12} />
				</Button>
			</Tooltip.Trigger>
			<Tooltip.Content
				>{loopEnabled ? "Loop on" : "Loop off"}</Tooltip.Content
			>
		</Tooltip.Root>

		<Tooltip.Root>
			<Tooltip.Trigger>
				<Button
					variant="ghost"
					size="icon-sm"
					onclick={toggleFullscreen}
					disabled={!fullscreenTargetEl}
					aria-pressed={isFullscreen}
					aria-label={isFullscreen
						? "Exit fullscreen"
						: "Enter fullscreen"}
					title={isFullscreen ? "Exit fullscreen" : "Enter fullscreen"}
				>
					{#if isFullscreen}
						<Minimize2 size={12} />
					{:else}
						<Maximize2 size={12} />
					{/if}
				</Button>
			</Tooltip.Trigger>
			<Tooltip.Content
				>{isFullscreen
					? "Exit fullscreen"
					: "Fullscreen (F)"}</Tooltip.Content
			>
		</Tooltip.Root>
	</div>
</div>
