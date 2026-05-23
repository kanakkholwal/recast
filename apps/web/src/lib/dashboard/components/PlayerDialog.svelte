<script lang="ts">
	import {
		formatBytes,
		formatDuration,
		formatRelative,
	} from "$lib/dashboard/format";
	import type { Recast } from "$lib/dashboard/store.svelte";
	import { Clock, Cloud, MonitorPlay, Video, X } from "@lucide/svelte";
	import { onMount } from "svelte";
	import { cubicOut } from "svelte/easing";
	import { fade, scale } from "svelte/transition";

	// Vidstack web components — Loom-style scrub thumbnails, speed menu, HLS
	// via hls.js, AirPlay/Cast for free. We import the player module + the
	// default video layout, then drop the custom elements into the template.
	// Styles imported eagerly so the layout has its CSS variables ready on
	// first paint (no FOUC). Vidstack auto-loads hls.js when the source URL
	// ends in `.m3u8`; progressive MP4 falls back to native <video>.
	import "vidstack/player";
	import "vidstack/player/ui";
	import "vidstack/player/layouts/default";
	import "vidstack/player/styles/default/theme.css";
	import "vidstack/player/styles/default/layouts/video.css";

	let {
		recast,
		onclose,
		onengagement,
	}: {
		recast: Recast;
		onclose: () => void;
		/**
		 * Engagement reporter. `event` is one of:
		 *   - `view-start` — playback actually started (autoplay or user hit play)
		 *   - `progress`   — fired every ~5s, `percent` is 0–100 watched
		 *   - `ended`      — viewer reached the end
		 * Caller wires these into the activity store / analytics.
		 */
		onengagement?: (event: "view-start" | "progress" | "ended", percent: number) => void;
	} = $props();

	let playerEl = $state<HTMLElement | null>(null);
	let lastReportedPct = $state(0);

	// Hook engagement events to the parent via custom-element listeners.
	// Done in onMount because the custom element isn't defined until after
	// the `vidstack/player` import has run on first render.
	onMount(() => {
		const el = playerEl;
		if (!el || !onengagement) return;

		let started = false;
		const onPlay = () => {
			if (started) return;
			started = true;
			onengagement("view-start", 0);
		};
		const onTime = (e: Event) => {
			const detail = (e as CustomEvent<{ currentTime: number }>).detail;
			const duration = recast.durationSec || 1;
			const pct = Math.min(100, Math.round((detail.currentTime / duration) * 100));
			// Throttle to ~5% steps so a 2-minute video doesn't spam 1k events.
			if (pct - lastReportedPct >= 5) {
				lastReportedPct = pct;
				onengagement("progress", pct);
			}
		};
		const onEnded = () => onengagement("ended", 100);

		el.addEventListener("play", onPlay);
		el.addEventListener("time-update", onTime);
		el.addEventListener("ended", onEnded);
		return () => {
			el.removeEventListener("play", onPlay);
			el.removeEventListener("time-update", onTime);
			el.removeEventListener("ended", onEnded);
		};
	});
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} />

<div class="fixed inset-0 z-100 grid place-items-center p-4 sm:p-8">
	<button
		type="button"
		aria-label="Close player"
		onclick={onclose}
		class="absolute inset-0 cursor-default bg-background/80 backdrop-blur-sm"
		transition:fade={{ duration: 150 }}
	></button>

	<div
		class="glass-card relative z-10 w-full max-w-3xl overflow-hidden rounded-2xl shadow-craft-xl"
		transition:scale={{ start: 0.96, duration: 240, easing: cubicOut }}
	>
		<header class="flex items-center gap-3 border-b border-border-low/50 px-4 py-3">
			<Video class="size-4 shrink-0 text-primary" />
			<span class="min-w-0 flex-1 truncate text-sm font-semibold text-foreground">
				{recast.title}
			</span>
			<button
				type="button"
				onclick={onclose}
				aria-label="Close"
				class="grid size-7 place-items-center rounded-md text-muted-foreground transition-colors hover:bg-foreground/8 hover:text-foreground"
			>
				<X class="size-4" />
			</button>
		</header>

		<!-- svelte-ignore a11y_media_has_caption -->
		<media-player
			bind:this={playerEl}
			src={recast.videoUrl}
			poster={recast.posterUrl || undefined}
			autoplay
			crossorigin
			class="aspect-video w-full bg-black"
			title={recast.title}
		>
			<media-provider></media-provider>
			<!-- `default-layout` is Vidstack's batteries-included controls: play,
			     scrubber with thumbnail preview (auto-generated from frames if
			     no VTT supplied), volume, captions menu, settings (speed,
			     quality), PiP, AirPlay, fullscreen. Theme overrides live in
			     [app.css :root.vidstack] when we get to brand polish. -->
			<media-video-layout></media-video-layout>
		</media-player>

		<footer class="flex flex-wrap items-center gap-x-4 gap-y-1 px-4 py-3 text-xs text-muted-foreground">
			<span class="flex items-center gap-1.5">
				<Clock class="size-3.5" />
				{formatDuration(recast.durationSec)}
			</span>
			<span>{formatBytes(recast.sizeBytes)}</span>
			<span>{formatRelative(recast.createdAt)}</span>
			<span class="flex items-center gap-1.5">
				{#if recast.source === "cloud"}
					<Cloud class="size-3.5 text-primary" />{recast.provider}
				{:else}
					<MonitorPlay class="size-3.5" />Local
				{/if}
			</span>
		</footer>
	</div>
</div>
