<script lang="ts">
	import {
	  Captions,
	  Maximize,
	  Minimize,
	  Pause,
	  PictureInPicture,
	  PictureInPicture2,
	  Play,
	  RotateCcw,
	  RotateCw,
	  Volume,
	  Volume1,
	  Volume2,
	  VolumeX
	} from "@lucide/svelte";
	import { onMount } from "svelte";
	import { fade } from "svelte/transition";
	import type {
	  RecastPlayerApi,
	  RecastPlayerBranding,
	  RecastPlayerChapter,
	  RecastPlayerControls,
	  RecastPlayerFeatures,
	  RecastPlayerMarker,
	  RecastPlayerProps,
	  RecastPlayerState,
	  RecastPlayerUtilityAction
	} from "./types";

	import "hls-video-element";
	import "media-chrome";

	const DEFAULT_CONTROLS: RecastPlayerControls = {
		bigPlay: true,
		seek: true,
		time: true,
		volume: true,
		playbackRate: true,
		captions: false,
		pip: true,
		fullscreen: true,
	};

	const DEFAULT_FEATURES: RecastPlayerFeatures = {
		settingsMenu: true,
		chaptersMenu: true,
		theaterMode: true,
		miniPlayer: true,
		share: true,
		download: true,
		screenshot: true,
		keyboardShortcuts: true,
		markers: true,
	};

	const DEFAULT_BRANDING_SRC = "/logo.svg";
	const DEFAULT_BRANDING: RecastPlayerBranding = {
		src: DEFAULT_BRANDING_SRC,
		alt: "Recast",
		name: "Recast",
		width: 118,
		height: 28,
		className: "",
	};

	const PLAYBACK_RATES = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 1.75, 2];

	let {
		src,
		poster = null,
		thumbnails = null,
		tracks = [],
		title = "",
		autoplay = false,
		preload = "metadata",
		crossorigin = "anonymous",
		loop = false,
		volume = $bindable(1),
		muted = $bindable(false),
		playbackRate = $bindable(1),
		currentTime = $bindable(0),
		paused = $bindable<boolean | null>(null),
		chapters = [],
		markers = [],
		utilityActions = [],
		features = {},
		showMenu = true,
		controls = {},
		branding = DEFAULT_BRANDING,
		aspectRatio = null,
		autohide = null,
		objectFit = "contain",
		ariaLabel = "",
		className = "",
		onengagement,
		onstatechange,
		onaction,
		api = $bindable<RecastPlayerApi | null>(null),
	}: RecastPlayerProps = $props();

	let controllerEl = $state<HTMLElement | null>(null);
	let videoEl = $state<HTMLVideoElement | null>(null);
	let lastReportedPct = 0;
	let started = false;
	let intrinsicWidth = $state(0);
	let intrinsicHeight = $state(0);
	let isTheaterMode = $state(false);
	let isPictureInPicture = $state(false);
	let activeTooltipId = $state<string | null>(null);

	const isHls = $derived(/\.m3u8(\?|#|$)/i.test(src));
	// media-chrome's `autohide` only suppresses the inactivity timer; the bar
	// still starts hidden on an autoplaying clip. `noautohide` on the control bar
	// is what keeps it visible from frame one (negative autohide = never hide).
	const pinControls = $derived(typeof autohide === "number" && autohide < 0);
	const mergedControls = $derived({ ...DEFAULT_CONTROLS, ...controls });
	const mergedFeatures = $derived({ ...DEFAULT_FEATURES, ...features });
	const showCaptions = $derived(mergedControls.captions && showMenu);
	const playerLabel = $derived(ariaLabel || title || "Video player");
	const resolvedBranding = $derived.by(() => {
		if (branding === null) return null;
		return { ...DEFAULT_BRANDING, ...branding };
	});
	const sortedChapters = $derived(
		[...chapters].sort((a, b) => a.startTime - b.startTime),
	);
	const resolvedUtilityActions = $derived.by(() => {
		if (utilityActions.length > 0) return utilityActions;
		const actions: RecastPlayerUtilityAction[] = [];
		if (mergedFeatures.share) actions.push({ id: "share", label: "Share" });
		if (mergedFeatures.screenshot) actions.push({ id: "screenshot", label: "Screenshot" });
		if (mergedFeatures.download) actions.push({ id: "download", label: "Download" });
		if (mergedFeatures.chaptersMenu && sortedChapters.length > 0) {
			actions.push({ id: "chapters", label: "Chapters" });
		}
		if (mergedFeatures.theaterMode) actions.push({ id: "theater", label: "Theater mode" });
		if (mergedFeatures.keyboardShortcuts) {
			actions.push({ id: "shortcuts", label: "Shortcuts" });
		}
		if (mergedFeatures.settingsMenu) actions.push({ id: "settings", label: "Settings" });
		return actions;
	});
	const activeChapter = $derived.by(() => {
		const current = currentTime;
		return (
			sortedChapters.find((chapter, index) => {
				const next = sortedChapters[index + 1];
				const endTime = chapter.endTime ?? next?.startTime ?? Number.POSITIVE_INFINITY;
				return current >= chapter.startTime && current < endTime;
			}) ?? null
		);
	});
	const resolvedAspectRatio = $derived.by(() => {
		if (typeof aspectRatio === "number" && aspectRatio > 0) return `${aspectRatio}`;
		if (typeof aspectRatio === "string" && aspectRatio.trim()) return aspectRatio.trim();
		if (intrinsicWidth > 0 && intrinsicHeight > 0) return `${intrinsicWidth} / ${intrinsicHeight}`;
		return null;
	});
	const playerStyle = $derived.by(() => {
		const vars = [
			// Reserve 16/9 before metadata loads; `auto` would collapse the
			// slotted <video> to 300×150 and cause a layout shift on the common case.
			resolvedAspectRatio
				? `--recast-player-aspect-ratio: ${resolvedAspectRatio};`
				: "--recast-player-aspect-ratio: 16 / 9;",
			`--recast-player-object-fit: ${objectFit};`,
		];
		return vars.join(" ");
	});

	function clamp01(value: number) {
		return Math.min(1, Math.max(0, value));
	}

	function getState(): RecastPlayerState {
		return {
			paused: videoEl?.paused ?? true,
			ended: videoEl?.ended ?? false,
			currentTime: videoEl?.currentTime ?? currentTime,
			duration: videoEl?.duration ?? 0,
			volume: videoEl?.volume ?? clamp01(volume),
			muted: videoEl?.muted ?? muted,
			playbackRate: videoEl?.playbackRate ?? playbackRate,
			videoWidth: videoEl?.videoWidth ?? intrinsicWidth,
			videoHeight: videoEl?.videoHeight ?? intrinsicHeight,
			pictureInPicture: isPictureInPicture,
			theaterMode: isTheaterMode,
		};
	}

	function emitState() {
		if (!onstatechange || !videoEl) return;
		onstatechange(getState());
	}

	async function safePlay() {
		if (!videoEl) return;
		try {
			await videoEl.play();
		} catch {
			paused = true;
			emitState();
		}
	}

	async function togglePlay() {
		if (!videoEl) return;
		if (videoEl.paused) await safePlay();
		else videoEl.pause();
	}

	function setTheaterMode(next: boolean) {
		isTheaterMode = next;
		onaction?.({ type: "theater", active: next });
		emitState();
	}

	async function enterFullscreen() {
		if (!controllerEl) return;
		if (document.fullscreenElement === controllerEl) return;
		await controllerEl.requestFullscreen?.();
	}

	async function exitFullscreen() {
		if (document.fullscreenElement) await document.exitFullscreen();
	}

	async function enterPictureInPicture() {
		if (!videoEl || !document.pictureInPictureEnabled) return;
		if (document.pictureInPictureElement === videoEl) return;
		await videoEl.requestPictureInPicture?.();
	}

	function chapterEndTime(chapter: RecastPlayerChapter, index: number) {
		return chapter.endTime ?? sortedChapters[index + 1]?.startTime ?? Number.POSITIVE_INFINITY;
	}

	function markerLeft(time: number) {
		const duration = videoEl?.duration ?? 0;
		if (!duration || !isFinite(duration)) return 0;
		return Math.max(0, Math.min(100, (time / duration) * 100));
	}

	function markerColor(marker: RecastPlayerMarker) {
		if (marker.color) return marker.color;
		switch (marker.kind) {
			case "comment":
				return "#60a5fa";
			case "highlight":
				return "#f59e0b";
			case "cta":
				return "#f43f5e";
			default:
				return "#cdec3a";
		}
	}

	function selectChapter(chapter: RecastPlayerChapter) {
		if (videoEl) videoEl.currentTime = Math.max(0, chapter.startTime);
		onaction?.({ type: "chapter-select", chapter });
	}

	function selectMarker(marker: RecastPlayerMarker) {
		if (videoEl) videoEl.currentTime = Math.max(0, marker.time);
		onaction?.({ type: "marker-select", marker });
	}

	function downloadVideo() {
		onaction?.({ type: "download", src });
		const anchor = document.createElement("a");
		anchor.href = src;
		anchor.download = title ? `${title}.mp4` : "video";
		anchor.rel = "noreferrer";
		anchor.click();
	}

	function shareVideo() {
		onaction?.({ type: "share", currentTime });
	}

	function captureScreenshotDataUrl() {
		if (!videoEl || !intrinsicWidth || !intrinsicHeight) return null;
		const canvas = document.createElement("canvas");
		canvas.width = intrinsicWidth;
		canvas.height = intrinsicHeight;
		const ctx = canvas.getContext("2d");
		if (!ctx) return null;
		ctx.drawImage(videoEl, 0, 0, intrinsicWidth, intrinsicHeight);
		return canvas.toDataURL("image/png");
	}

	function screenshotVideo() {
		const dataUrl = captureScreenshotDataUrl();
		if (!dataUrl) return;
		onaction?.({ type: "screenshot", currentTime, dataUrl });
	}

	async function handleUtilityAction(action: RecastPlayerUtilityAction) {
		switch (action.id) {
			case "share":
				shareVideo();
				break;
			case "download":
				downloadVideo();
				break;
			case "screenshot":
				screenshotVideo();
				break;
			case "theater":
				setTheaterMode(!isTheaterMode);
				break;
			case "pip":
				await enterPictureInPicture();
				break;
			case "custom":
				onaction?.({ type: "custom", actionId: action.actionId, currentTime });
				break;
		}
	}

	function utilityLabel(action: RecastPlayerUtilityAction) {
		return action.label ?? action.id;
	}

	function showTooltip(id: string) {
		activeTooltipId = id;
	}

	function hideTooltip(id: string) {
		if (activeTooltipId === id) activeTooltipId = null;
	}

	onMount(() => {
		api = {
			play: safePlay,
			pause: () => videoEl?.pause(),
			togglePlay,
			seek: (seconds) => {
				if (videoEl) videoEl.currentTime = Math.max(0, seconds);
			},
			setMuted: (next) => {
				if (videoEl) videoEl.muted = next;
			},
			setVolume: (next) => {
				if (videoEl) videoEl.volume = clamp01(next);
			},
			setPlaybackRate: (next) => {
				if (videoEl) videoEl.playbackRate = next;
			},
			setTheaterMode,
			openSettings: () => {},
			closeSettings: () => {},
			enterFullscreen,
			exitFullscreen,
			enterPictureInPicture,
			getCurrentTime: () => videoEl?.currentTime ?? 0,
			getDuration: () => videoEl?.duration ?? 0,
			getState,
			getVideoElement: () => videoEl,
		};
		return () => {
			api = null;
		};
	});

	function handlePlay() {
		paused = false;
		if (!started && onengagement) {
			started = true;
			onengagement({ type: "view-start", percent: 0 });
		}
		emitState();
	}

	function handlePause() {
		paused = true;
		emitState();
	}

	function handleLoadedMetadata() {
		if (!videoEl) return;
		intrinsicWidth = videoEl.videoWidth;
		intrinsicHeight = videoEl.videoHeight;
		currentTime = videoEl.currentTime;
		emitState();
	}

	function handleTimeUpdate() {
		if (!videoEl) return;
		currentTime = videoEl.currentTime;
		emitState();
		if (!onengagement) return;
		const duration = videoEl.duration || 0;
		if (!duration || !isFinite(duration)) return;
		const pct = Math.min(100, Math.round((videoEl.currentTime / duration) * 100));
		if (pct - lastReportedPct >= 5) {
			lastReportedPct = pct;
			onengagement({ type: "progress", percent: pct, currentTime: videoEl.currentTime });
		}
	}

	function handleVolumeChange() {
		if (!videoEl) return;
		volume = videoEl.volume;
		muted = videoEl.muted;
		emitState();
	}

	function handleRateChange() {
		if (!videoEl) return;
		playbackRate = videoEl.playbackRate;
		emitState();
	}

	function handleSeeked() {
		if (!videoEl) return;
		currentTime = videoEl.currentTime;
		emitState();
	}

	function handleEnded() {
		if (!videoEl) return;
		paused = true;
		emitState();
		if (!onengagement) return;
		onengagement({ type: "ended", percent: 100, currentTime: videoEl.currentTime });
	}

	function handleEnterPictureInPicture() {
		isPictureInPicture = true;
		emitState();
	}

	function handleLeavePictureInPicture() {
		isPictureInPicture = false;
		emitState();
	}

	// PiP events aren't in Svelte's typed video attributes; bind them directly.
	$effect(() => {
		const el = videoEl;
		if (!el) return;
		el.addEventListener("enterpictureinpicture", handleEnterPictureInPicture);
		el.addEventListener("leavepictureinpicture", handleLeavePictureInPicture);
		return () => {
			el.removeEventListener("enterpictureinpicture", handleEnterPictureInPicture);
			el.removeEventListener("leavepictureinpicture", handleLeavePictureInPicture);
		};
	});

	function handleKeyDown(event: KeyboardEvent) {
		if (!videoEl) return;
		const target = event.target as HTMLElement | null;
		if (
			target &&
			(target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable)
		) {
			return;
		}
		switch (event.key) {
			case " ":
			case "k":
			case "K":
				event.preventDefault();
				void togglePlay();
				break;
			case "ArrowLeft":
			case "j":
			case "J":
				event.preventDefault();
				videoEl.currentTime = Math.max(0, videoEl.currentTime - 5);
				break;
			case "ArrowRight":
			case "l":
			case "L":
				event.preventDefault();
				videoEl.currentTime = Math.min(
					videoEl.duration || Number.MAX_SAFE_INTEGER,
					videoEl.currentTime + 5,
				);
				break;
			case "m":
			case "M":
				event.preventDefault();
				videoEl.muted = !videoEl.muted;
				break;
			case "f":
			case "F":
				event.preventDefault();
				if (document.fullscreenElement) void exitFullscreen();
				else void enterFullscreen();
				break;
			case "c":
			case "C":
			case "?":
			case "Escape":
				break;
			case "Home":
				event.preventDefault();
				videoEl.currentTime = 0;
				break;
			case "End":
				if (isFinite(videoEl.duration)) {
					event.preventDefault();
					videoEl.currentTime = videoEl.duration;
				}
				break;
		}
	}

	$effect(() => {
		if (!videoEl) return;
		const next = clamp01(volume);
		if (Math.abs(videoEl.volume - next) > 0.01) videoEl.volume = next;
	});

	$effect(() => {
		if (!videoEl) return;
		if (videoEl.muted !== muted) videoEl.muted = muted;
	});

	$effect(() => {
		if (!videoEl) return;
		if (Math.abs(videoEl.playbackRate - playbackRate) > 0.001) {
			videoEl.playbackRate = playbackRate;
		}
	});

	$effect(() => {
		if (!videoEl) return;
		if (paused === null) return;
		if (paused && !videoEl.paused) videoEl.pause();
		if (!paused && videoEl.paused) void safePlay();
	});

	$effect(() => {
		if (!videoEl || !isFinite(currentTime)) return;
		if (Math.abs(videoEl.currentTime - currentTime) > 0.05) {
			videoEl.currentTime = Math.max(0, currentTime);
		}
	});
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<media-controller
	bind:this={controllerEl}
	class={`recast-player ${isTheaterMode ? "recast-player-theater" : ""} ${className}`}
	style={playerStyle}
	aria-label={playerLabel}
	role="region"
	tabindex="0"
	defaultsubtitles
	autohide={autohide ?? undefined}
	onkeydown={handleKeyDown}
>
	{#if isHls}
		<!-- svelte-ignore a11y_media_has_caption -->
		<hls-video
			bind:this={videoEl}
			slot="media"
			class="recast-media"
			{src}
			{poster}
			{title}
			{preload}
			{loop}
			crossorigin={crossorigin ?? undefined}
			playsinline
			{autoplay}
			onplay={handlePlay}
			onpause={handlePause}
			onloadedmetadata={handleLoadedMetadata}
			ontimeupdate={handleTimeUpdate}
			onvolumechange={handleVolumeChange}
			onratechange={handleRateChange}
			onseeked={handleSeeked}
			onended={handleEnded}
		>
			{#if thumbnails}
				<track kind="metadata" src={thumbnails} label="thumbnails" default />
			{/if}
			{#each tracks as track (track.src)}
				<track
					src={track.src}
					kind={track.kind}
					label={track.label}
					srclang={track.srclang}
					default={track.default}
				/>
			{/each}
		</hls-video>
	{:else}
		<!-- svelte-ignore a11y_media_has_caption -->
		<video
			bind:this={videoEl}
			slot="media"
			class="recast-media"
			{src}
			{poster}
			{title}
			{preload}
			{loop}
			crossorigin={crossorigin ?? undefined}
			playsinline
			{autoplay}
			onplay={handlePlay}
			onpause={handlePause}
			onloadedmetadata={handleLoadedMetadata}
			ontimeupdate={handleTimeUpdate}
			onvolumechange={handleVolumeChange}
			onratechange={handleRateChange}
			onseeked={handleSeeked}
			onended={handleEnded}
		>
			{#if thumbnails}
				<track kind="metadata" src={thumbnails} label="thumbnails" default />
			{/if}
			{#each tracks as track (track.src)}
				<track
					src={track.src}
					kind={track.kind}
					label={track.label}
					srclang={track.srclang}
					default={track.default}
				/>
			{/each}
		</video>
	{/if}

	<media-loading-indicator class="recast-loading"></media-loading-indicator>

	{#if resolvedBranding?.src}
		{#if resolvedBranding.href}
			<a
				class={`recast-branding ${resolvedBranding.className ?? ""}`}
				href={resolvedBranding.href}
				target="_blank"
				rel="noreferrer"
				aria-label={resolvedBranding.alt}
			>
				<span class="recast-branding-mark">
					<img
						src={resolvedBranding.src}
						alt={resolvedBranding.alt}
						width={resolvedBranding.width}
						height={resolvedBranding.height}
					/>
				</span>
				{#if resolvedBranding.name}
					<span class="recast-branding-name">{resolvedBranding.name}</span>
				{/if}
			</a>
		{:else}
			<div
				class={`recast-branding ${resolvedBranding.className ?? ""}`}
				aria-hidden="true"
			>
				<span class="recast-branding-mark">
					<img
						src={resolvedBranding.src}
						alt={resolvedBranding.alt}
						width={resolvedBranding.width}
						height={resolvedBranding.height}
					/>
				</span>
				{#if resolvedBranding.name}
					<span class="recast-branding-name">{resolvedBranding.name}</span>
				{/if}
			</div>
		{/if}
	{/if}

	{#if mergedControls.bigPlay}
		<media-play-button class="recast-big-play" aria-label="Toggle playback">
			<span slot="play" class="recast-icon recast-icon-big">
				<Play class="size-7 translate-x-px" />
			</span>
			<span slot="pause" class="recast-icon recast-icon-big">
				<Pause class="size-7" />
			</span>
		</media-play-button>
	{/if}

	<media-control-bar class="recast-control-bar" noautohide={pinControls ? "" : undefined}>
		<!-- Floating glass pill: detached from the video edges, frosted, with the
		     scrubber running full-width along the top and the transport / utility
		     controls split into left + right groups on the row below. -->
		<div class="recast-pill">
			<div class="recast-scrubber-wrap">
				<media-time-range class="recast-scrubber" aria-label="Seek">
					{#if thumbnails}
						<media-preview-thumbnail slot="preview" class="recast-thumb"></media-preview-thumbnail>
					{/if}
					<media-preview-time-display slot="preview" class="recast-preview-time"></media-preview-time-display>
				</media-time-range>

				{#if mergedFeatures.markers && markers.length > 0}
					<div class="recast-marker-rail">
						{#each markers as marker (marker.id)}
							{@const markerTooltipId = `marker-${marker.id}`}
							<button
								type="button"
								class="recast-marker"
								style={`left:${markerLeft(marker.time)}%;--recast-marker-color:${markerColor(marker)};`}
								title={marker.label}
								aria-label={marker.label}
								onmouseenter={() => showTooltip(markerTooltipId)}
								onmouseleave={() => hideTooltip(markerTooltipId)}
								onfocus={() => showTooltip(markerTooltipId)}
								onblur={() => hideTooltip(markerTooltipId)}
								onclick={() => selectMarker(marker)}
							></button>
							{#if activeTooltipId === markerTooltipId}
								<div
									class="recast-ui-tooltip recast-ui-tooltip-marker"
									style={`left:${markerLeft(marker.time)}%;`}
									transition:fade={{ duration: 120 }}
								>
									{marker.label}
								</div>
							{/if}
						{/each}
					</div>
				{/if}
			</div>

			<div class="recast-pill-row">
				<div class="recast-group">
					<media-play-button class="recast-btn" aria-label="Play or pause">
						<span slot="play" class="recast-icon"><Play class="size-4 translate-x-[0.5px]" /></span>
						<span slot="pause" class="recast-icon"><Pause class="size-4" /></span>
					</media-play-button>

					{#if mergedControls.seek}
						<media-seek-backward-button class="recast-btn recast-btn-seek" seekoffset="10" aria-label="Back 10 seconds">
							<span slot="icon" class="recast-icon"><RotateCcw class="size-4" /></span>
						</media-seek-backward-button>

						<media-seek-forward-button class="recast-btn recast-btn-seek" seekoffset="10" aria-label="Forward 10 seconds">
							<span slot="icon" class="recast-icon"><RotateCw class="size-4" /></span>
						</media-seek-forward-button>
					{/if}

					{#if mergedControls.time}
						<media-time-display class="recast-time" showduration></media-time-display>
					{/if}
				</div>

				<div class="recast-group recast-group-end">
					{#if mergedControls.volume}
						<media-mute-button class="recast-btn" aria-label="Mute or unmute">
							<span slot="off" class="recast-icon"><VolumeX class="size-4" /></span>
							<span slot="low" class="recast-icon"><Volume class="size-4" /></span>
							<span slot="medium" class="recast-icon"><Volume1 class="size-4" /></span>
							<span slot="high" class="recast-icon"><Volume2 class="size-4" /></span>
						</media-mute-button>
						<media-volume-range class="recast-volume" aria-label="Volume"></media-volume-range>
					{/if}

					{#if mergedControls.playbackRate}
						<media-playback-rate-button
							class="recast-btn recast-btn-text"
							rates="0.25 0.5 0.75 1 1.25 1.5 1.75 2"
							aria-label="Playback speed"
						></media-playback-rate-button>
					{/if}

					{#if showCaptions}
						<media-captions-button class="recast-btn" aria-label="Captions">
							<span slot="on" class="recast-icon"><Captions class="size-4" /></span>
							<span slot="off" class="recast-icon recast-icon-muted"><Captions class="size-4" /></span>
						</media-captions-button>
					{/if}

					{#if mergedControls.pip}
						<media-pip-button class="recast-btn" aria-label="Picture in picture">
							<span slot="enter" class="recast-icon"><PictureInPicture class="size-4" /></span>
							<span slot="exit" class="recast-icon"><PictureInPicture2 class="size-4" /></span>
						</media-pip-button>
					{/if}

					{#if mergedControls.fullscreen}
						<media-fullscreen-button class="recast-btn" aria-label="Fullscreen">
							<span slot="enter" class="recast-icon"><Maximize class="size-4" /></span>
							<span slot="exit" class="recast-icon"><Minimize class="size-4" /></span>
						</media-fullscreen-button>
					{/if}
				</div>
			</div>
		</div>
	</media-control-bar>
</media-controller>

<style>
	:global(.recast-player) {
		display: block;
		width: 100%;
		aspect-ratio: var(--recast-player-aspect-ratio, auto);
		background: #000;
	}

	:global(.recast-player .recast-media) {
		width: 100%;
		height: 100%;
		object-fit: var(--recast-player-object-fit, contain);
		background: #000;
	}

	.recast-branding {
		position: absolute;
		top: 14px;
		left: 14px;
		z-index: 3;
		display: inline-flex;
		align-items: center;
		gap: 0;
		min-height: 40px;
		padding: 6px;
		border-radius: 999px;
		background: rgba(15, 15, 14, 0.42);
		color: rgba(255, 255, 255, 0.96);
		backdrop-filter: blur(16px) saturate(145%);
		-webkit-backdrop-filter: blur(16px) saturate(145%);
		box-shadow: 0 8px 18px rgba(0, 0, 0, 0.22);
		text-decoration: none;
		transition:
			padding 180ms ease,
			gap 180ms ease,
			background-color 180ms ease;
	}

	.recast-branding-mark {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		flex: 0 0 auto;
		width: 28px;
		height: 28px;
		border-radius: 999px;
		background: rgba(255, 255, 255, 0.14);
		overflow: hidden;
	}

	.recast-branding img {
		display: block;
		height: auto;
		width: 18px;
		max-width: 18px;
		max-height: 18px;
		object-fit: contain;
	}

	.recast-branding-name {
		max-width: 0;
		overflow: hidden;
		opacity: 0;
		transform: translateX(-4px);
		transition:
			max-width 180ms ease,
			opacity 160ms ease,
			transform 180ms ease;
		font-size: 12px;
		font-weight: 600;
		line-height: 1;
		letter-spacing: 0;
		white-space: nowrap;
	}

	.recast-branding:hover,
	.recast-branding:focus-visible {
		gap: 10px;
		padding-right: 12px;
		background: rgba(15, 15, 14, 0.5);
	}

	.recast-branding:hover .recast-branding-name,
	.recast-branding:focus-visible .recast-branding-name {
		max-width: 120px;
		opacity: 1;
		transform: translateX(0);
	}

	.recast-ui-tooltip {
		position: absolute;
		z-index: 6;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		padding: 6px 12px;
		border-radius: 6px;
		background: var(--foreground, #111111);
		color: var(--background, #ffffff);
		box-shadow: var(--shadow-craft-sm);
		font-size: 12px;
		font-weight: 500;
		line-height: 1.2;
		letter-spacing: 0;
		white-space: nowrap;
		pointer-events: none;
	}

	.recast-ui-tooltip::after {
		content: "";
		position: absolute;
		left: 50%;
		top: 100%;
		width: 10px;
		height: 10px;
		background: inherit;
		transform: translate(-50%, -55%) rotate(45deg);
		border-radius: 2px;
	}

	.recast-ui-tooltip-marker {
		bottom: calc(100% + 10px);
		transform: translateX(-50%);
	}
</style>
