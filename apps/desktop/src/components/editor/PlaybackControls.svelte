<script lang="ts">
	import { Button } from "@recast/ui/button";
	import * as DropdownMenu from "@recast/ui/dropdown-menu";
	import * as Tooltip from "@recast/ui/tooltip";
	import type { EditorStore } from "$lib/stores/editor-store.svelte";
	import {
		Gauge,
		Maximize2,
		Minimize2,
		Pause,
		Play,
		Repeat,
		SkipBack,
		SkipForward,
		ZoomIn,
		ZoomOut,
	} from "@lucide/svelte";

	interface Props {
		store: EditorStore;
		videoEl?: HTMLVideoElement | null;
		/** Element to request fullscreen on (usually the preview container). */
		fullscreenTargetEl?: HTMLElement | null;
	}

	let { store, videoEl = null, fullscreenTargetEl = null }: Props = $props();

	const SPEEDS = [0.25, 0.5, 1.0, 1.5, 2.0];
	let playbackSpeed = $state(1.0);
	let loopEnabled = $state(false);
	let isFullscreen = $state(false);

	// Keep the video element's playbackRate synced with the chosen speed.
	$effect(() => {
		if (videoEl) videoEl.playbackRate = playbackSpeed;
	});

	// Loop: at trimEnd, seek back to trimStart instead of letting the clip end.
	$effect(() => {
		if (!videoEl || !loopEnabled) return;
		const onTime = () => {
			if (!videoEl) return;
			const effectiveEnd = store.trimEnd || store.metadata?.duration || Infinity;
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
	const durationFormatted = $derived(formatTime(store.metadata?.duration ?? 0));
	const trimSummary = $derived(
		`${formatTime(store.trimStart)} – ${formatTime(store.trimEnd || store.metadata?.duration || 0)}`,
	);
	const hasTrim = $derived(
		store.trimStart > 0 ||
			((store.metadata?.duration ?? 0) > 0 && store.trimEnd < (store.metadata?.duration ?? 0)),
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
			Math.min(videoEl.currentTime + frameDuration * direction, store.metadata.duration),
		);
		store.currentTime = videoEl.currentTime;
	}

	function zoomTimeline(dir: number) {
		store.timelineZoom = Math.max(0.5, Math.min(5, store.timelineZoom + dir * 0.25));
	}
</script>

<div
	class="flex h-9 shrink-0 items-center justify-between gap-3 border-t border-border bg-card/40 px-3 text-[11px]"
>
	<div class="flex min-w-0 items-center gap-2">
		<span class="font-mono tabular-nums font-medium text-foreground">
			{currentTimeFormatted}
		</span>
		<span class="text-muted-foreground/40">/</span>
		<span class="font-mono tabular-nums text-muted-foreground">{durationFormatted}</span>
		{#if hasTrim}
			<span
				class="inline-flex items-center rounded border border-success/20 bg-success/10 px-1.5 py-0.5 text-[10px] font-medium text-success"
			>
				Trim {trimSummary}
			</span>
		{/if}
	</div>

	<div class="flex items-center gap-0.5">
		<Tooltip.Root>
			<Tooltip.Trigger>
				<Button
					variant="ghost"
					size="icon-sm"
					onclick={() => stepFrame(-1)}
					aria-label="Previous frame"
				>
					<SkipBack size={13} />
				</Button>
			</Tooltip.Trigger>
			<Tooltip.Content>Previous frame (←)</Tooltip.Content>
		</Tooltip.Root>

		<Tooltip.Root>
			<Tooltip.Trigger>
				<Button
					variant="default"
					size="icon-sm"
					onclick={togglePlay}
					aria-label={store.isPlaying ? "Pause" : "Play"}
					class="size-7"
				>
					{#if store.isPlaying}
						<Pause size={13} fill="currentColor" />
					{:else}
						<Play size={13} fill="currentColor" class="ml-0.5" />
					{/if}
				</Button>
			</Tooltip.Trigger>
			<Tooltip.Content>{store.isPlaying ? "Pause" : "Play"} (Space)</Tooltip.Content>
		</Tooltip.Root>

		<Tooltip.Root>
			<Tooltip.Trigger>
				<Button
					variant="ghost"
					size="icon-sm"
					onclick={() => stepFrame(1)}
					aria-label="Next frame"
				>
					<SkipForward size={13} />
				</Button>
			</Tooltip.Trigger>
			<Tooltip.Content>Next frame (→)</Tooltip.Content>
		</Tooltip.Root>
	</div>

	<div class="flex items-center gap-1 text-muted-foreground">
		<!-- Speed menu -->
		<DropdownMenu.Root>
			<DropdownMenu.Trigger>
				<Button
					variant="ghost"
					size="xs"
					class="gap-1 font-mono tabular-nums"
					aria-label="Playback speed"
				>
					<Gauge size={12} />
					{playbackSpeed.toFixed(2).replace(/\.?0+$/, "")}×
				</Button>
			</DropdownMenu.Trigger>
			<DropdownMenu.Content align="end">
				{#each SPEEDS as speed (speed)}
					<DropdownMenu.Item
						onclick={() => (playbackSpeed = speed)}
						class={playbackSpeed === speed ? "text-primary" : ""}
					>
						{speed.toFixed(2).replace(/\.?0+$/, "")}×
					</DropdownMenu.Item>
				{/each}
			</DropdownMenu.Content>
		</DropdownMenu.Root>

		<Tooltip.Root>
			<Tooltip.Trigger>
				<Button
					variant={loopEnabled ? "default_soft" : "ghost"}
					size="icon-sm"
					onclick={() => (loopEnabled = !loopEnabled)}
					aria-pressed={loopEnabled}
					aria-label="Loop playback within trim"
				>
					<Repeat size={13} />
				</Button>
			</Tooltip.Trigger>
			<Tooltip.Content>{loopEnabled ? "Loop on" : "Loop off"}</Tooltip.Content>
		</Tooltip.Root>

		<Tooltip.Root>
			<Tooltip.Trigger>
				<Button
					variant="ghost"
					size="icon-sm"
					onclick={toggleFullscreen}
					disabled={!fullscreenTargetEl}
					aria-pressed={isFullscreen}
					aria-label={isFullscreen ? "Exit fullscreen" : "Enter fullscreen"}
				>
					{#if isFullscreen}
						<Minimize2 size={13} />
					{:else}
						<Maximize2 size={13} />
					{/if}
				</Button>
			</Tooltip.Trigger>
			<Tooltip.Content>{isFullscreen ? "Exit fullscreen" : "Fullscreen (F)"}</Tooltip.Content>
		</Tooltip.Root>

		<span class="mx-1 h-4 w-px bg-border"></span>

		<Tooltip.Root>
			<Tooltip.Trigger>
				<Button
					variant="ghost"
					size="icon-sm"
					onclick={() => zoomTimeline(-1)}
					aria-label="Zoom out timeline"
				>
					<ZoomOut size={13} />
				</Button>
			</Tooltip.Trigger>
			<Tooltip.Content>Zoom out</Tooltip.Content>
		</Tooltip.Root>

		<span class="min-w-9 text-center font-mono tabular-nums text-foreground">
			{store.timelineZoom.toFixed(1)}x
		</span>

		<Tooltip.Root>
			<Tooltip.Trigger>
				<Button
					variant="ghost"
					size="icon-sm"
					onclick={() => zoomTimeline(1)}
					aria-label="Zoom in timeline"
				>
					<ZoomIn size={13} />
				</Button>
			</Tooltip.Trigger>
			<Tooltip.Content>Zoom in</Tooltip.Content>
		</Tooltip.Root>
	</div>
</div>
