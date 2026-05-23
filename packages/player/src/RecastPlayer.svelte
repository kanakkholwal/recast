<script lang="ts">
	import { onMount } from "svelte";
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
	 * Volume bind callback — Svelte 5 can't `bind:volume` to a custom
	 * element, so we set the attribute imperatively after mount. Done in
	 * onMount AND when the prop changes via $effect for re-renders.
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
			autoplay={autoplay ? "" : undefined}
			onplay={handlePlay}
			ontimeupdate={handleTimeUpdate}
			onended={handleEnded}
		>
			{#if thumbnails}
				<!-- media-chrome reads scrub-preview tiles from this metadata
				     track. The VTT file points at a sprite image (Cloudinary
				     can generate one per upload). -->
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
			autoplay={autoplay ? "" : undefined}
			onplay={handlePlay}
			ontimeupdate={handleTimeUpdate}
			onended={handleEnded}
		>
			{#if thumbnails}
				<track kind="metadata" src={thumbnails} label="thumbnails" default />
			{/if}
		</video>
	{/if}

	<!-- Loading spinner — auto-hides once enough buffer is ready. -->
	<media-loading-indicator slot="centered-chrome" noautohide></media-loading-indicator>

	<!-- Center play overlay — shows pre-play, hides during playback. -->
	<media-play-button slot="centered-chrome" class="big-play"></media-play-button>

	<!-- Bottom control bar. Editor surfaces can drop sibling rows above
	     this one (comment lane, cut-marker rail) without fighting any
	     pre-built layout — this whole bar is our own composition. -->
	<media-control-bar>
		<media-play-button></media-play-button>
		<media-seek-backward-button seekoffset="10"></media-seek-backward-button>
		<media-seek-forward-button seekoffset="10"></media-seek-forward-button>
		<media-time-display showduration></media-time-display>
		<media-time-range>
			{#if thumbnails}
				<media-preview-thumbnail slot="preview"></media-preview-thumbnail>
			{/if}
			<media-preview-time-display slot="preview"></media-preview-time-display>
		</media-time-range>
		<media-mute-button></media-mute-button>
		<media-volume-range></media-volume-range>
		<!-- Cycles 1x → 1.5x → 2x → 0.5x → 1x on click. No menu, one button —
		     keeps the bar dense and avoids speculative menu elements. -->
		<media-playback-rate-button rates="0.5 1 1.25 1.5 2"></media-playback-rate-button>
		{#if showMenu}
			<media-captions-button></media-captions-button>
		{/if}
		<media-pip-button></media-pip-button>
		<media-fullscreen-button></media-fullscreen-button>
	</media-control-bar>
</media-controller>

<style>
	/* Local fallbacks — actual theme lives in styles.css (consumed by the
	   host app once at root). These ensure the player still renders
	   reasonably even if the consumer forgot the stylesheet import. */
	:global(.recast-player) {
		display: block;
		width: 100%;
		aspect-ratio: 16 / 9;
		background: #000;
		--media-primary-color: #cdec3a;
	}

	:global(.recast-player .big-play) {
		--media-control-background: transparent;
		--media-button-icon-height: 56px;
	}
</style>
