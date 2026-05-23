<script lang="ts">
	import { onMount } from "svelte";
	import {
		Captions,
		Maximize,
		Minimize,
		PictureInPicture,
		PictureInPicture2,
		Pause,
		Play,
		RotateCcw,
		RotateCw,
		Volume,
		Volume1,
		Volume2,
		VolumeX,
	} from "@lucide/svelte";
	import type { RecastPlayerApi, RecastPlayerProps } from "./types";

	/**
	 * Side-effect imports — register the custom elements. `hls-video-element`
	 * defines `<hls-video>`, which speaks HLS via hls.js and falls back to
	 * native MP4 for non-`.m3u8` sources. `media-chrome` registers every
	 * `<media-*>` element we slot in below.
	 */
	import "media-chrome";
	import "hls-video-element";

	let {
		src,
		poster = null,
		thumbnails = null,
		title = "",
		autoplay = false,
		volume = 1,
		showMenu = true,
		className = "",
		onengagement,
		api = $bindable<RecastPlayerApi | null>(null),
	}: RecastPlayerProps = $props();

	let videoEl = $state<HTMLVideoElement | null>(null);
	let lastReportedPct = 0;
	let started = false;

	// Whether to use <hls-video> or plain <video>. `.m3u8` URLs need HLS.js;
	// MP4/WebM streams play natively. Recomputing on src change so a swap
	// between cloud-HLS and a local blob URL doesn't strand us in the wrong
	// element.
	const isHls = $derived(/\.m3u8(\?|#|$)/i.test(src));

	onMount(() => {
		// Publish the imperative API once mounted. Editor surfaces hold this
		// reference and call seek() / pause() / etc. from transcript clicks,
		// AI cut buttons, comment timeline interactions.
		api = {
			play: async () => {
				if (videoEl) await videoEl.play();
			},
			pause: () => videoEl?.pause(),
			seek: (seconds) => {
				if (videoEl) videoEl.currentTime = Math.max(0, seconds);
			},
			getCurrentTime: () => videoEl?.currentTime ?? 0,
			getDuration: () => videoEl?.duration ?? 0,
			getVideoElement: () => videoEl,
		};
		return () => {
			api = null;
		};
	});

	function handlePlay() {
		if (started || !onengagement) return;
		started = true;
		onengagement({ type: "view-start", percent: 0 });
	}

	function handleTimeUpdate() {
		if (!onengagement || !videoEl) return;
		const duration = videoEl.duration || 0;
		if (!duration || !isFinite(duration)) return;
		const pct = Math.min(100, Math.round((videoEl.currentTime / duration) * 100));
		// Throttle to ~5% steps. A 5-minute video sends ~20 events instead
		// of ~3000 — enough granularity for "watched %" without burying the
		// analytics consumer in noise.
		if (pct - lastReportedPct >= 5) {
			lastReportedPct = pct;
			onengagement({
				type: "progress",
				percent: pct,
				currentTime: videoEl.currentTime,
			});
		}
	}

	function handleEnded() {
		if (!onengagement || !videoEl) return;
		onengagement({
			type: "ended",
			percent: 100,
			currentTime: videoEl.currentTime,
		});
	}

	/**
	 * Volume sync — Svelte 5 can't `bind:volume` to a custom element, so we
	 * write through to the underlying media element via $effect.
	 */
	$effect(() => {
		if (videoEl) videoEl.volume = Math.min(1, Math.max(0, volume));
	});
</script>

<media-controller class={`recast-player ${className}`} defaultsubtitles>
	{#if isHls}
		<!-- svelte-ignore a11y_media_has_caption -->
		<hls-video
			bind:this={videoEl}
			slot="media"
			{src}
			{poster}
			{title}
			crossorigin="anonymous"
			playsinline
			{autoplay}
			onplay={handlePlay}
			ontimeupdate={handleTimeUpdate}
			onended={handleEnded}
		>
			{#if thumbnails}
				<track kind="metadata" src={thumbnails} label="thumbnails" default />
			{/if}
		</hls-video>
	{:else}
		<!-- svelte-ignore a11y_media_has_caption -->
		<video
			bind:this={videoEl}
			slot="media"
			{src}
			{poster}
			{title}
			crossorigin="anonymous"
			playsinline
			{autoplay}
			onplay={handlePlay}
			ontimeupdate={handleTimeUpdate}
			onended={handleEnded}
		>
			{#if thumbnails}
				<track kind="metadata" src={thumbnails} label="thumbnails" default />
			{/if}
		</video>
	{/if}

	<!-- Loading spinner — appears only while buffering. The default
	     autohide window (~500ms buffer-stall) is what we want; removing
	     it would leave a permanent spinner over a happily-playing video. -->
	<media-loading-indicator class="recast-loading"></media-loading-indicator>

	<!-- Big center play overlay. Positioned absolutely (rather than via
	     media-chrome's centered-chrome slot) so it sits at the actual
	     visual center of the video frame instead of being flex-stacked
	     next to other centered-chrome children. Auto-hides during
	     playback via the `mediapaused`-driven rule in styles.css. -->
	<media-play-button class="recast-big-play">
		<span slot="play" class="recast-icon recast-icon-big">
			<Play class="size-7 translate-x-px" />
		</span>
		<span slot="pause" class="recast-icon recast-icon-big">
			<Pause class="size-7" />
		</span>
	</media-play-button>

	<!-- Bottom control bar. Custom-icon slots throughout so every button
	     uses Lucide (the rest of the app's icon vocabulary) — no
	     media-chrome default glyphs leak in. -->
	<media-control-bar class="recast-control-bar">
		<media-play-button class="recast-btn">
			<span slot="play" class="recast-icon"><Play class="size-4 translate-x-[0.5px]" /></span>
			<span slot="pause" class="recast-icon"><Pause class="size-4" /></span>
		</media-play-button>

		<media-seek-backward-button class="recast-btn" seekoffset="10">
			<span slot="icon" class="recast-icon"><RotateCcw class="size-4" /></span>
		</media-seek-backward-button>

		<media-seek-forward-button class="recast-btn" seekoffset="10">
			<span slot="icon" class="recast-icon"><RotateCw class="size-4" /></span>
		</media-seek-forward-button>

		<media-time-display class="recast-time" showduration></media-time-display>

		<media-time-range class="recast-scrubber">
			{#if thumbnails}
				<media-preview-thumbnail
					slot="preview"
					class="recast-thumb"
				></media-preview-thumbnail>
			{/if}
			<media-preview-time-display
				slot="preview"
				class="recast-preview-time"
			></media-preview-time-display>
		</media-time-range>

		<media-mute-button class="recast-btn">
			<span slot="off" class="recast-icon"><VolumeX class="size-4" /></span>
			<span slot="low" class="recast-icon"><Volume class="size-4" /></span>
			<span slot="medium" class="recast-icon"><Volume1 class="size-4" /></span>
			<span slot="high" class="recast-icon"><Volume2 class="size-4" /></span>
		</media-mute-button>

		<media-volume-range class="recast-volume"></media-volume-range>

		<media-playback-rate-button
			class="recast-btn recast-btn-text"
			rates="0.5 1 1.25 1.5 2"
		></media-playback-rate-button>

		{#if showMenu}
			<media-captions-button class="recast-btn">
				<span slot="on" class="recast-icon"><Captions class="size-4" /></span>
				<span slot="off" class="recast-icon recast-icon-muted">
					<Captions class="size-4" />
				</span>
			</media-captions-button>
		{/if}

		<media-pip-button class="recast-btn">
			<span slot="enter" class="recast-icon"><PictureInPicture class="size-4" /></span>
			<span slot="exit" class="recast-icon"><PictureInPicture2 class="size-4" /></span>
		</media-pip-button>

		<media-fullscreen-button class="recast-btn">
			<span slot="enter" class="recast-icon"><Maximize class="size-4" /></span>
			<span slot="exit" class="recast-icon"><Minimize class="size-4" /></span>
		</media-fullscreen-button>
	</media-control-bar>
</media-controller>

<style>
	/* Local minimum so the controller has dimensions even if the host app
	   forgot to import @recast/player/styles.css. Full theming lives in
	   that stylesheet so consumers can override per-app if they need. */
	:global(.recast-player) {
		display: block;
		width: 100%;
		aspect-ratio: 16 / 9;
		background: #000;
	}
</style>
