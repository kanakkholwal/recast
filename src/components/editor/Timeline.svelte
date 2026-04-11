<script lang="ts">
	import type { EditorStore } from "$lib/stores/editor-store.svelte";
	import { Scissors, Search, X } from "@lucide/svelte";
	import { onMount } from "svelte";
	import { cubicOut } from "svelte/easing";
	import { fade, fly, slide } from "svelte/transition";

	interface Props {
		store: EditorStore;
		videoEl?: HTMLVideoElement | null;
	}

	let { store, videoEl = null }: Props = $props();

	let timelineEl: HTMLDivElement | undefined = $state();
	let isDraggingPlayhead = $state(false);
	let showTrimHelp = $state(false);
	let timelineWidth = $state(900);

	const duration = $derived(store.metadata?.duration ?? 0);
	const pixelsPerSecond = $derived(
		duration > 0 ? (timelineWidth * store.timelineZoom) / duration : 100,
	);
	const totalWidth = $derived(Math.max(duration * pixelsPerSecond, timelineWidth));
	const playheadLeft = $derived(store.currentTime * pixelsPerSecond);
	const clipLeft = $derived(store.trimStart * pixelsPerSecond);
	const clipRight = $derived((store.trimEnd || duration) * pixelsPerSecond);
	const clipWidth = $derived(Math.max(clipRight - clipLeft, 0));
	const thumbnailWidth = $derived(
		store.thumbnailStrip.length > 0
			? Math.max(88, clipWidth / store.thumbnailStrip.length)
			: 112,
	);
	const hasTrim = $derived(
		store.trimStart > 0 ||
			((store.metadata?.duration ?? 0) > 0 &&
				store.trimEnd > 0 &&
				store.trimEnd < (store.metadata?.duration ?? 0)),
	);
	const frameCount = $derived(
		Math.max(0, Math.round((store.metadata?.duration ?? 0) * (store.metadata?.fps ?? 0))),
	);
	const aspectRatioLabel = $derived.by(() => {
		const width = store.metadata?.width ?? 0;
		const height = store.metadata?.height ?? 0;
		if (!width || !height) return "Source";
		const divisor = greatestCommonDivisor(width, height);
		return `${Math.round(width / divisor)}:${Math.round(height / divisor)}`;
	});

	const timeMarkers = $derived.by(() => {
		if (duration <= 0) return [];
		const markers: { time: number; label: string; emphasis: boolean }[] = [];

		let interval = 1;
		if (pixelsPerSecond < 26) interval = 10;
		else if (pixelsPerSecond < 52) interval = 5;
		else if (pixelsPerSecond < 120) interval = 2;
		else if (pixelsPerSecond > 260) interval = 0.5;

		for (let t = 0; t <= duration + interval * 0.5; t += interval) {
			const mins = Math.floor(t / 60);
			const secs = Math.floor(t % 60);
			markers.push({
				time: t,
				label: `${mins}:${secs.toString().padStart(2, "0")}`,
				emphasis: Math.round(t) % (interval >= 2 ? interval * 2 : 2) === 0,
			});
		}

		return markers;
	});

	const minorTicks = $derived.by(() => {
		if (duration <= 0) return [];
		const ticks: number[] = [];
		const interval = pixelsPerSecond > 180 ? 0.25 : pixelsPerSecond > 80 ? 0.5 : 1;
		for (let t = 0; t <= duration + interval * 0.5; t += interval) {
			ticks.push(t);
		}
		return ticks;
	});

	function greatestCommonDivisor(a: number, b: number): number {
		let left = Math.abs(a);
		let right = Math.abs(b);
		while (right !== 0) {
			const next = left % right;
			left = right;
			right = next;
		}
		return left || 1;
	}

	function formatTime(seconds: number) {
		const mins = Math.floor(seconds / 60);
		const secs = Math.floor(seconds % 60);
		const centiseconds = Math.floor((seconds % 1) * 100);
		return `${mins}:${secs.toString().padStart(2, "0")}.${centiseconds
			.toString()
			.padStart(2, "0")}`;
	}

	function seekToPosition(clientX: number) {
		if (!timelineEl || duration <= 0) return;
		const rect = timelineEl.getBoundingClientRect();
		const scrollLeft = timelineEl.scrollLeft;
		const x = clientX - rect.left + scrollLeft;
		const time = Math.max(0, Math.min(duration, x / pixelsPerSecond));
		store.currentTime = time;
		if (videoEl) videoEl.currentTime = time;
	}

	function handleTimelinePointerDown(event: PointerEvent) {
		isDraggingPlayhead = true;
		(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
		seekToPosition(event.clientX);
	}

	function handleTimelinePointerMove(event: PointerEvent) {
		if (!isDraggingPlayhead) return;
		seekToPosition(event.clientX);
	}

	function handleTimelinePointerUp() {
		isDraggingPlayhead = false;
	}

	function handleTimelineKeydown(event: KeyboardEvent) {
		if (duration <= 0) return;

		const step = event.shiftKey ? 1 : store.metadata?.fps ? 1 / store.metadata.fps : 1 / 30;

		if (event.key === "ArrowLeft") {
			event.preventDefault();
			const next = Math.max(0, store.currentTime - step);
			store.currentTime = next;
			if (videoEl) videoEl.currentTime = next;
		}

		if (event.key === "ArrowRight") {
			event.preventDefault();
			const next = Math.min(duration, store.currentTime + step);
			store.currentTime = next;
			if (videoEl) videoEl.currentTime = next;
		}
	}

	function handleResize() {
		if (!timelineEl) return;
		timelineWidth = timelineEl.clientWidth;
	}

	function handleTimelineWheel(event: WheelEvent) {
		if (!timelineEl) return;

		if (event.ctrlKey || event.metaKey) {
			event.preventDefault();
			const rect = timelineEl.getBoundingClientRect();
			const anchorX = event.clientX - rect.left;
			const anchorTime =
				duration > 0
					? (timelineEl.scrollLeft + anchorX) / pixelsPerSecond
					: 0;
			const delta = event.deltaY < 0 ? 0.2 : -0.2;
			const nextZoom = Math.max(0.5, Math.min(5, store.timelineZoom + delta));
			if (nextZoom === store.timelineZoom) return;
			store.timelineZoom = nextZoom;
			requestAnimationFrame(() => {
				if (!timelineEl || duration <= 0) return;
				const nextPixelsPerSecond = (timelineEl.clientWidth * nextZoom) / duration;
				timelineEl.scrollLeft = Math.max(
					0,
					anchorTime * nextPixelsPerSecond - anchorX,
				);
			});
			return;
		}

		if (Math.abs(event.deltaY) > Math.abs(event.deltaX)) {
			event.preventDefault();
			timelineEl.scrollLeft += event.deltaY;
		}
	}

	function syncVideoTime() {
		if (!videoEl) return;
		videoEl.currentTime = Math.max(0, Math.min(duration, store.currentTime));
	}

	function addFocusRegion() {
		if (duration <= 0) return;
		const clipEnd = store.trimEnd || duration;
		const start = Math.max(store.trimStart, store.currentTime - 0.35);
		const end = Math.min(clipEnd, Math.max(start + 0.8, store.currentTime + 0.85));
		store.addZoomRegion(start, end, 1.8);
	}

	function setTrimPoint(kind: "in" | "out") {
		if (duration <= 0) return;
		const minGap = 0.1;
		if (kind === "in") {
			const nextIn = Math.min(store.currentTime, Math.max(0, (store.trimEnd || duration) - minGap));
			store.trimStart = nextIn;
			if (store.currentTime < nextIn) {
				store.currentTime = nextIn;
			}
		} else {
			const nextOut = Math.max(store.currentTime, Math.min(duration, store.trimStart + minGap));
			store.trimEnd = nextOut;
			if (store.currentTime > nextOut) {
				store.currentTime = nextOut;
			}
		}
		syncVideoTime();
	}

	function resetTrim() {
		store.trimStart = 0;
		store.trimEnd = duration;
		syncVideoTime();
	}

	onMount(() => {
		handleResize();
		const observer = new ResizeObserver(handleResize);
		if (timelineEl) observer.observe(timelineEl);
		return () => observer.disconnect();
	});
</script>

<div class="shrink-0 select-none border-t border-border bg-card/30 px-3 pb-2 pt-2">
	<div class="mb-2 flex flex-wrap items-center justify-between gap-2 text-[11px]">
		<div class="flex items-center gap-1">
			<button
				type="button"
				class="rounded-md border border-border bg-background px-2 py-1 font-medium text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
				onclick={() => setTrimPoint("in")}
			>
				Set In
			</button>
			<button
				type="button"
				class="rounded-md border border-border bg-background px-2 py-1 font-medium text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
				onclick={() => setTrimPoint("out")}
			>
				Set Out
			</button>
			<button
				type="button"
				class="inline-flex items-center gap-1.5 rounded-md border border-border bg-background px-2 py-1 font-medium text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
				onclick={addFocusRegion}
			>
				<Search size={11} />
				Focus
			</button>
			{#if hasTrim}
				<button
					type="button"
					class="inline-flex items-center gap-1.5 rounded-md border border-success/20 bg-success/10 px-2 py-1 font-medium text-success transition-colors hover:bg-success/20"
					onclick={resetTrim}
				>
					<Scissors size={11} />
					Reset Trim
				</button>
			{/if}
		</div>

		<div class="flex items-center gap-1.5 text-muted-foreground">
			<span class="rounded border border-border bg-background px-1.5 py-0.5 font-mono tabular-nums text-foreground">
				{aspectRatioLabel}
			</span>
			<span class="rounded border border-border bg-background px-1.5 py-0.5 font-mono tabular-nums text-foreground">
				{frameCount} frames
			</span>
			<button
				type="button"
				class="rounded border border-border bg-background px-1.5 py-0.5 font-medium text-muted-foreground transition-colors hover:text-foreground"
				onclick={() => (showTrimHelp = !showTrimHelp)}
			>
				Help
			</button>
			<span class="inline-flex items-center gap-1">
				<kbd class="rounded border border-border bg-background px-1 py-0.5 font-mono text-[10px]"
					>Scroll</kbd
				>
				<span>pan</span>
			</span>
			<span class="inline-flex items-center gap-1">
				<kbd class="rounded border border-border bg-background px-1 py-0.5 font-mono text-[10px]"
					>⌘ Scroll</kbd
				>
				<span>zoom</span>
			</span>
		</div>
	</div>

	{#if showTrimHelp}
		<div
			in:slide={{ duration: 160 }}
			out:fade={{ duration: 120 }}
			class="mb-2 rounded-md border border-border bg-background px-2.5 py-1.5 text-[11px] text-muted-foreground"
		>
			Set <span class="text-foreground">In</span> and <span class="text-foreground">Out</span> at the playhead to shorten the clip.
			The shaded segment is what will render and export.
		</div>
	{/if}

	<div
		bind:this={timelineEl}
		role="slider"
		tabindex="0"
		aria-label="Timeline scrubber"
		aria-valuemin={0}
		aria-valuemax={duration}
		aria-valuenow={store.currentTime}
		class="custom-scrollbar relative overflow-x-auto overflow-y-hidden rounded-lg border border-border bg-background/60"
		onpointerdown={handleTimelinePointerDown}
		onpointermove={handleTimelinePointerMove}
		onpointerup={handleTimelinePointerUp}
		onpointercancel={handleTimelinePointerUp}
		onwheel={handleTimelineWheel}
		onkeydown={handleTimelineKeydown}
	>
		<div class="relative min-w-full" style="width: {totalWidth}px; height: 156px;">
			<div class="relative h-7 border-b border-border bg-muted/20">
				{#each minorTicks as tick}
					<div
						class="absolute bottom-0 w-px bg-border/50"
						style="left: {tick * pixelsPerSecond}px; height: 5px;"
					></div>
				{/each}

				{#each timeMarkers as marker}
					<div
						class="absolute top-0 flex h-full flex-col items-start"
						style="left: {marker.time * pixelsPerSecond}px;"
					>
						<div
							class="w-px bg-border"
							style="height: {marker.emphasis ? '10px' : '6px'};"
						></div>
						<span
							class="mt-0.5 -translate-x-1/2 font-mono tabular-nums text-[10px] text-muted-foreground/80"
						>
							{marker.label}
						</span>
					</div>
				{/each}
			</div>

			<div class="relative px-2 pb-2 pt-1.5">
				<div class="relative h-12 rounded-md border border-border bg-background">
					<div
						class="absolute inset-y-0 rounded-md border border-primary/40 bg-primary/5"
						style="left: {clipLeft}px; width: {clipWidth}px;"
					>
						<div class="absolute inset-0 overflow-hidden rounded-md">
							{#if store.thumbnailStrip.length > 0}
								<div class="flex h-full">
									{#each store.thumbnailStrip as frame, index (frame + index)}
										<img
											in:fade={{ duration: 180 }}
											src={frame}
											alt="Timeline frame"
											class="h-full shrink-0 object-cover"
											style="width: {thumbnailWidth}px;"
											draggable="false"
										/>
									{/each}
								</div>
							{:else}
								<div
									class="flex h-full items-center justify-center text-[10px] text-muted-foreground"
								>
									Generating thumbnails…
								</div>
							{/if}
						</div>

						<div
							class="absolute left-2 top-1 rounded border border-border bg-background/80 px-1.5 py-0.5 font-mono text-[9px] text-muted-foreground backdrop-blur"
						>
							{hasTrim ? "Trimmed" : "Full clip"}
						</div>
						<div class="absolute inset-y-0 left-0 w-1 rounded-l-md bg-primary"></div>
						<div class="absolute inset-y-0 right-0 w-1 rounded-r-md bg-primary"></div>
					</div>
				</div>

				<div class="mt-1.5 min-h-9 rounded-md border border-border bg-background/40 px-1.5 py-1.5">
					{#if store.zoomRegions.length === 0}
						<div
							class="flex h-6 items-center justify-center text-[10px] text-muted-foreground"
						>
							Add a focus region to punch in during playback
						</div>
					{:else}
						{#each store.zoomRegions as region, index (region.id)}
							<div
								in:fly={{ y: 10, duration: 180, easing: cubicOut }}
								out:fade={{ duration: 140 }}
								class="absolute overflow-hidden rounded border border-border bg-muted aspect-video"
								style="
									left: {region.start * pixelsPerSecond}px;
									width: {Math.max((region.end - region.start) * pixelsPerSecond, 56)}px;
									top: {86 + index * 2}px;
									height: 30px;
								"
							>
								<div class="flex h-full items-center justify-between gap-2 px-2">
									<div class="min-w-0">
										<p class="text-[10px] font-semibold text-foreground">Focus</p>
										<p class="text-[9px] text-muted-foreground">
											{region.scale.toFixed(1)}x · {formatTime(region.start)}
										</p>
									</div>
									<button
										type="button"
										onclick={(event) => {
											event.stopPropagation();
											store.removeZoomRegion(region.id);
										}}
										class="flex h-4 w-4 items-center justify-center rounded border border-border text-muted-foreground transition-colors hover:border-destructive hover:text-destructive"
										aria-label="Remove focus region"
									>
										<X size={9} strokeWidth={2.5} />
									</button>
								</div>
							</div>
						{/each}
					{/if}
				</div>
			</div>

			<div
				class="absolute top-0 z-30 transition-[left] ease-out"
				style="left: {playheadLeft}px; transition-duration: {isDraggingPlayhead ? '0ms' : '90ms'};"
			>
				<div class="relative -translate-x-1/2">
					<div
						class="absolute left-1/2 top-1 -translate-x-1/2 rounded border border-border bg-foreground px-1.5 py-0.5 font-mono text-[9px] tabular-nums text-background"
					>
						{formatTime(store.currentTime)}
					</div>
					<div class="mx-auto mt-6 size-2 rounded-full border border-background bg-primary"></div>
					<div class="mx-auto h-[126px] w-px bg-primary/60"></div>
				</div>
			</div>
		</div>
	</div>
</div>

<style>
	.custom-scrollbar::-webkit-scrollbar {
		height: 8px;
	}

	.custom-scrollbar::-webkit-scrollbar-track {
		background: transparent;
	}

	.custom-scrollbar::-webkit-scrollbar-thumb {
		background: rgba(120, 120, 128, 0.35);
		border-radius: 999px;
	}

	.custom-scrollbar {
		scrollbar-width: thin;
		scrollbar-color: rgba(120, 120, 128, 0.35) transparent;
	}
</style>
