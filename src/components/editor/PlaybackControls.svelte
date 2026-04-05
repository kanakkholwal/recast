<script lang="ts">
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
		`${formatTime(store.trimStart)} - ${formatTime(store.trimEnd || store.metadata?.duration || 0)}`,
	);
	const hasTrim = $derived(
		store.trimStart > 0 || (store.metadata?.duration ?? 0) > 0 && store.trimEnd < (store.metadata?.duration ?? 0),
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

<div class="shrink-0 border-t border-border/70 bg-card/30 px-4 py-2 backdrop-blur-sm">
	<div class="flex flex-wrap items-center justify-between gap-3">
		<div class="flex items-center gap-2 text-sm">
			<span class="font-mono font-medium tabular-nums text-foreground">
				{currentTimeFormatted}
			</span>
			<span class="text-muted-foreground/50">/</span>
			<span class="font-mono tabular-nums text-muted-foreground">{durationFormatted}</span>
			{#if hasTrim}
				<span class="rounded-full border border-emerald-500/25 bg-emerald-500/10 px-2 py-0.5 text-[10px] font-medium text-emerald-200">
					Trim {trimSummary}
				</span>
			{/if}
		</div>

		<div class="flex items-center gap-1 rounded-full border border-border/70 bg-background/75 p-1 shadow-sm">
			<Tooltip.Root>
				<Tooltip.Trigger>
					<button
						type="button"
						onclick={() => stepFrame(-1)}
						class="flex h-8 w-8 items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
						aria-label="Previous frame"
					>
						<SkipBack size={16} />
					</button>
				</Tooltip.Trigger>
				<Tooltip.Content>Previous frame</Tooltip.Content>
			</Tooltip.Root>

			<Tooltip.Root>
				<Tooltip.Trigger>
					<button
						type="button"
						onclick={togglePlay}
						class="flex h-10 w-10 items-center justify-center rounded-full bg-foreground text-background shadow-sm transition-transform hover:scale-[1.02] active:scale-95"
						aria-label={store.isPlaying ? "Pause" : "Play"}
					>
						{#if store.isPlaying}
							<Pause size={18} fill="currentColor" />
						{:else}
							<Play size={18} fill="currentColor" class="ml-0.5" />
						{/if}
					</button>
				</Tooltip.Trigger>
				<Tooltip.Content>{store.isPlaying ? "Pause" : "Play"} (Space)</Tooltip.Content>
			</Tooltip.Root>

			<Tooltip.Root>
				<Tooltip.Trigger>
					<button
						type="button"
						onclick={() => stepFrame(1)}
						class="flex h-8 w-8 items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
						aria-label="Next frame"
					>
						<SkipForward size={16} />
					</button>
				</Tooltip.Trigger>
				<Tooltip.Content>Next frame</Tooltip.Content>
			</Tooltip.Root>
		</div>

		<div class="flex items-center gap-1 rounded-full border border-border/70 bg-background/75 px-2 py-1 text-[11px] text-muted-foreground shadow-sm">
			<Tooltip.Root>
				<Tooltip.Trigger>
					<button
						type="button"
						onclick={() => zoomTimeline(-1)}
						class="flex h-7 w-7 items-center justify-center rounded-full transition-colors hover:bg-muted hover:text-foreground"
						aria-label="Zoom out timeline"
					>
						<ZoomOut size={14} />
					</button>
				</Tooltip.Trigger>
				<Tooltip.Content>Zoom out</Tooltip.Content>
			</Tooltip.Root>

			<span class="min-w-10 text-center font-mono tabular-nums text-foreground">
				{store.timelineZoom.toFixed(1)}x
			</span>

			<Tooltip.Root>
				<Tooltip.Trigger>
					<button
						type="button"
						onclick={() => zoomTimeline(1)}
						class="flex h-7 w-7 items-center justify-center rounded-full transition-colors hover:bg-muted hover:text-foreground"
						aria-label="Zoom in timeline"
					>
						<ZoomIn size={14} />
					</button>
				</Tooltip.Trigger>
				<Tooltip.Content>Zoom in</Tooltip.Content>
			</Tooltip.Root>
		</div>
	</div>
</div>
