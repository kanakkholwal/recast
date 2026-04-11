<script lang="ts">
	import { Button } from "$components/ui/button";
	import * as Tooltip from "$components/ui/tooltip";
	import type { EditorStore } from "$lib/stores/editor-store.svelte";
	import { Pause, Play, SkipBack, SkipForward, ZoomIn, ZoomOut } from "@lucide/svelte";

	interface Props {
		store: EditorStore;
		videoEl?: HTMLVideoElement | null;
	}

	let { store, videoEl = null }: Props = $props();

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

	<div class="flex items-center gap-0.5 text-muted-foreground">
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
