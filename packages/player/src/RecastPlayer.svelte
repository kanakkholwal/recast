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
	// @recast/icons maps Lucide-style names onto Tabler glyphs, and Tabler's
	// volume scale runs the other way: `Volume` is the two-arc loud one and
	// `Volume2` (IconVolume3) draws no arcs at all. Aliased to what they
	// actually render so the level mapping below can't be read backwards.
	Volume as VolumeHigh,
	Volume1 as VolumeMedium,
	Volume2 as VolumeLow,
	VolumeX as VolumeMuted,
} from "@recast/icons";
import { onMount } from "svelte";
import { fade } from "svelte/transition";
import type {
	RecastPlayerApi,
	RecastPlayerBranding,
	RecastPlayerControls,
	RecastPlayerMarker,
	RecastPlayerProps,
	RecastPlayerState,
} from "./types";

import "hls-video-element";
import "media-chrome";
import "media-chrome/menu";

import {
	activeChunkIndex,
	activeWordIndex,
	chunkWords,
	DEFAULT_CAPTION_STYLE,
	parseKaraokeCue,
	resolveCaptionAnimation,
	spokenWordCount,
	type TranscriptWord,
} from "@recast/captions";
import CaptionBox from "@recast/captions/box";
import {
	EngagementTracker,
	markerLeftPct,
	RECAST_BRANDING,
	resolveDownloadPlan,
	volumeLevel,
} from "./player.logic";

// Minimal by default: play, time, then speed / volume / fullscreen. The +-10s
// jog buttons and PiP are opt-in — the scrubber covers the same intent and a
// six-button row was reading as a toolbar bolted onto the video.
const DEFAULT_CONTROLS: RecastPlayerControls = {
	bigPlay: true,
	seek: false,
	time: true,
	volume: true,
	playbackRate: true,
	captions: true,
	pip: false,
	fullscreen: true,
};

const PLAYBACK_RATES = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 1.75, 2];

let {
	src,
	poster = null,
	thumbnails = null,
	tracks = [],
	captionStyle = {},
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
	markers = [],
	controls = {},
	keyboardShortcuts = true,
	branding = null,
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
let intrinsicWidth = $state(0);
let intrinsicHeight = $state(0);
// Mirrors videoEl.duration into reactive state: the DOM property itself never
// invalidates, so marker positions computed from it would stay frozen at 0.
let duration = $state(0);
let isTheaterMode = $state(false);
let isPictureInPicture = $state(false);
let activeTooltipId = $state<string | null>(null);
const engagement = new EngagementTracker();

const rateMenuId = $props.id();

const isHls = $derived(/\.m3u8(\?|#|$)/i.test(src));
// media-chrome's `autohide` only suppresses the inactivity timer; the bar
// still starts hidden on an autoplaying clip. `noautohide` on the control bar
// is what keeps it visible from frame one (negative autohide = never hide).
const pinControls = $derived(typeof autohide === "number" && autohide < 0);
const mergedControls = $derived({ ...DEFAULT_CONTROLS, ...controls });
const playerLabel = $derived(ariaLabel || title || "Video player");
const mediaTag = $derived(isHls ? "hls-video" : "video");

// Mute is a plain button, not `media-mute-button`: media-chrome swaps its
// off/low/medium/high icons via shadow-DOM rules keyed on a
// `mediavolumelevel` attribute the controller does not propagate reliably
// here. Deriving from our own state is deterministic.
const VolumeIcon = $derived(
	{
		muted: VolumeMuted,
		low: VolumeLow,
		medium: VolumeMedium,
		high: VolumeHigh,
	}[volumeLevel(volume, muted)],
);

function toggleMute() {
	if (!videoEl) return;
	videoEl.muted = !videoEl.muted;
}

// ── Styled caption overlay ──────────────────────────────────────────────
// Renders captions through the shared @recast/captions CaptionBox (the same
// look as the editor) instead of the browser's default cue boxes: word-by-word
// highlight when the VTT carries inline timestamps, else the whole cue.
const resolvedCaptionStyle = $derived({ ...DEFAULT_CAPTION_STYLE, ...captionStyle });
const captionAnim = $derived(resolveCaptionAnimation(resolvedCaptionStyle.animation));
const hasCaptionTrack = $derived(
	tracks.some((t) => t.kind === "captions" || t.kind === "subtitles"),
);
let captionsEnabled = $state(true);
let cueWords = $state<TranscriptWord[]>([]);

// The chunk to show at the playhead + its progress, mirroring the editor's
// CaptionOverlay. Times are output-time seconds (the uploaded VTT is output-
// time-mapped), matching `currentTime`.
const captionView = $derived.by(() => {
	if (!captionsEnabled || cueWords.length === 0) return null;
	const runs = chunkWords(cueWords, captionAnim);
	const ci = activeChunkIndex(runs, currentTime);
	const chunk = runs[ci];
	if (!chunk) return null;
	return {
		key: `${chunk.start}:${ci}`,
		words: chunk.words,
		spoken: spokenWordCount(chunk.words, currentTime),
		wi: activeWordIndex(chunk.words, currentTime, captionAnim.holdGaps),
	};
});
const captionVertical = $derived(
	resolvedCaptionStyle.position === "center"
		? "top: 50%; transform: translateY(-50%);"
		: resolvedCaptionStyle.position === "top"
			? `top: ${resolvedCaptionStyle.offsetPct}%;`
			: `bottom: ${resolvedCaptionStyle.offsetPct}%;`,
);
const captionJustify = $derived(
	resolvedCaptionStyle.align === "left"
		? "flex-start"
		: resolvedCaptionStyle.align === "right"
			? "flex-end"
			: "center",
);

// Bind the caption track: keep it "hidden" (cues stay parsed, but the UA never
// paints its default boxes — our overlay renders instead) and refresh the
// active cue's words on every cue change.
$effect(() => {
	const video = videoEl;
	if (!video || !hasCaptionTrack) return;
	let track: TextTrack | null = null;
	const readActive = () => {
		if (!track) return;
		// Re-assert hidden so a stray "showing" (e.g. the `default` track attr)
		// can't double-render native cues over our overlay.
		if (track.mode === "showing") track.mode = "hidden";
		const cue = track.activeCues?.[0] as VTTCue | undefined;
		cueWords = cue ? parseKaraokeCue(cue.text, cue.startTime, cue.endTime) : [];
	};
	const attach = () => {
		const found = Array.from(video.textTracks).find(
			(t) => t.kind === "captions" || t.kind === "subtitles",
		);
		if (!found || found === track) return;
		track?.removeEventListener("cuechange", readActive);
		track = found;
		track.mode = "hidden";
		track.addEventListener("cuechange", readActive);
		readActive();
	};
	attach();
	video.textTracks.addEventListener("addtrack", attach);
	return () => {
		video.textTracks.removeEventListener("addtrack", attach);
		track?.removeEventListener("cuechange", readActive);
		cueWords = [];
	};
});

const resolvedBranding = $derived.by(() => {
	if (branding === null) return null;
	return { ...RECAST_BRANDING, ...branding };
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
	// iOS Safari has no Element.requestFullscreen; the video element owns
	// fullscreen there and only in its own native presentation mode.
	if (typeof controllerEl.requestFullscreen !== "function") {
		(videoEl as unknown as { webkitEnterFullscreen?: () => void })?.webkitEnterFullscreen?.();
		return;
	}
	await controllerEl.requestFullscreen();
}

async function exitFullscreen() {
	if (document.fullscreenElement === controllerEl) await document.exitFullscreen();
}

async function enterPictureInPicture() {
	if (!videoEl || !document.pictureInPictureEnabled) return;
	if (document.pictureInPictureElement === videoEl) return;
	await videoEl.requestPictureInPicture?.();
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

function selectMarker(marker: RecastPlayerMarker) {
	if (videoEl) videoEl.currentTime = Math.max(0, marker.time);
	onaction?.({ type: "marker-select", marker });
}

async function download() {
	const plan = resolveDownloadPlan(src, title, window.location.origin);
	onaction?.({ type: "download", src });
	const anchor = document.createElement("a");
	anchor.download = plan.filename;
	anchor.rel = "noreferrer";
	let objectUrl: string | null = null;
	if (plan.strategy === "fetch-blob") {
		// A cross-origin `download` attribute is ignored and the anchor
		// navigates instead, so the bytes have to come through us.
		try {
			const response = await fetch(src, { mode: "cors", credentials: "omit" });
			if (!response.ok) throw new Error(`${response.status}`);
			objectUrl = URL.createObjectURL(await response.blob());
		} catch {
			window.open(src, "_blank", "noopener,noreferrer");
			return;
		}
	}
	anchor.href = objectUrl ?? src;
	document.body.append(anchor);
	anchor.click();
	anchor.remove();
	if (objectUrl) URL.revokeObjectURL(objectUrl);
}

function captureScreenshot() {
	if (!videoEl || !intrinsicWidth || !intrinsicHeight) return null;
	const canvas = document.createElement("canvas");
	canvas.width = intrinsicWidth;
	canvas.height = intrinsicHeight;
	const ctx = canvas.getContext("2d");
	if (!ctx) return null;
	try {
		ctx.drawImage(videoEl, 0, 0, intrinsicWidth, intrinsicHeight);
		// Throws SecurityError when the media isn't CORS-clean.
		return canvas.toDataURL("image/png");
	} catch {
		return null;
	}
}

function screenshot() {
	const dataUrl = captureScreenshot();
	if (!dataUrl) return null;
	onaction?.({ type: "screenshot", currentTime, dataUrl });
	return dataUrl;
}

function reload() {
	mediaError = null;
	videoEl?.load();
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
		enterFullscreen,
		exitFullscreen,
		enterPictureInPicture,
		download,
		captureScreenshot,
		reload,
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
	const event = engagement.onPlay();
	if (event) onengagement?.(event);
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
	handleDurationChange();
	emitState();
}

function handleDurationChange() {
	const next = videoEl?.duration ?? 0;
	duration = Number.isFinite(next) ? next : 0;
}

function handleTimeUpdate() {
	if (!videoEl) return;
	currentTime = videoEl.currentTime;
	emitState();
	if (!onengagement) return;
	const event = engagement.onTimeUpdate(videoEl.currentTime, videoEl.duration || 0);
	if (event) onengagement(event);
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
	onengagement?.(engagement.onEnded(videoEl.currentTime));
}

let mediaError = $state<MediaError | null>(null);

function handleError() {
	mediaError = videoEl?.error ?? null;
}

const errorMessage = $derived.by(() => {
	if (!mediaError) return null;
	switch (mediaError.code) {
		case MediaError.MEDIA_ERR_NETWORK:
			return "The connection dropped while loading this video.";
		case MediaError.MEDIA_ERR_DECODE:
			return "This video couldn't be decoded on this device.";
		case MediaError.MEDIA_ERR_SRC_NOT_SUPPORTED:
			return "This video is unavailable — the link may have expired.";
		default:
			return "Playback was interrupted.";
	}
});

// Reset per-session state when the source changes, otherwise a reused
// component keeps the previous clip's view-start and progress high-water mark.
$effect(() => {
	src;
	engagement.reset();
	mediaError = null;
	duration = 0;
});

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

// Home/End only. Every other transport key belongs to <media-controller>'s
// own hotkeys, which already handle range-focus and per-button opt-out —
// duplicating them here made each press fire twice.
function handleKeyDown(event: KeyboardEvent) {
	if (!videoEl || !keyboardShortcuts) return;
	const target = event.target as HTMLElement | null;
	if (
		target &&
		(target.tagName === "INPUT" ||
			target.tagName === "TEXTAREA" ||
			target.isContentEditable ||
			target.tagName.startsWith("MEDIA-"))
	) {
		return;
	}
	if (event.key === "Home") {
		event.preventDefault();
		videoEl.currentTime = 0;
	} else if (event.key === "End" && Number.isFinite(videoEl.duration)) {
		event.preventDefault();
		videoEl.currentTime = videoEl.duration;
	}
}

// Bound imperatively: media-chrome owns keydown on the controller, so a
// template handler here just trips Svelte's non-interactive-element rule.
$effect(() => {
	const el = controllerEl;
	if (!el) return;
	el.addEventListener("keydown", handleKeyDown);
	return () => el.removeEventListener("keydown", handleKeyDown);
});

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

<media-controller
	bind:this={controllerEl}
	class={`recast-player ${isTheaterMode ? "recast-player-theater" : ""} ${className}`}
	style={playerStyle}
	aria-label={playerLabel}
	role="region"
	nohotkeys={keyboardShortcuts ? undefined : ""}
	autohide={autohide ?? undefined}
>
	<!-- svelte-ignore a11y_media_has_caption -->
	<svelte:element
		this={mediaTag}
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
		ondurationchange={handleDurationChange}
		ontimeupdate={handleTimeUpdate}
		onvolumechange={handleVolumeChange}
		onratechange={handleRateChange}
		onseeked={handleSeeked}
		onended={handleEnded}
		onerror={handleError}
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
	</svelte:element>

	<media-loading-indicator class="recast-loading"></media-loading-indicator>

	{#if mediaError}
		<!-- `noautohide` or media-chrome fades the error away with the control bar. -->
		<div class="recast-error" {...{ noautohide: "" }}>
			<p class="recast-error-message" role="alert">{errorMessage}</p>
			<button type="button" class="recast-error-retry" onclick={reload}>
				<RotateCcw class="size-3.5" />
				Try again
			</button>
		</div>
	{/if}

	{#if captionView}
		<!-- `noautohide`: media-chrome fades every slotted controller child on
		     inactivity EXCEPT slot=media/poster, role=dialog, or [noautohide].
		     Captions must stay until toggled off. -->
		<div class="recast-caption-layer" {...{ noautohide: "" }}>
			<div
				class="recast-caption-slot"
				class:recast-caption-bottom={resolvedCaptionStyle.position === "bottom"}
				style="{captionVertical} justify-content: {captionJustify};"
			>
				{#key captionView.key}
					<CaptionBox
						words={captionView.words}
						style={resolvedCaptionStyle}
						anim={captionAnim}
						spokenCount={captionView.spoken}
						activeIndex={captionView.wi}
						fontSize="{resolvedCaptionStyle.fontSizePct}cqh"
					/>
				{/key}
			</div>
		</div>
	{/if}

	{#if resolvedBranding?.src}
		{#if resolvedBranding.href}
			<a
				class={`recast-branding ${resolvedBranding.className ?? ""}`}
				href={resolvedBranding.href}
				target="_blank"
				rel="noreferrer"
			>
				<span class="recast-branding-mark">
					<img
						src={resolvedBranding.src}
						alt=""
						width={resolvedBranding.width}
						height={resolvedBranding.height}
					/>
				</span>
				{#if resolvedBranding.name}
					<span class="recast-branding-name">{resolvedBranding.name}</span>
				{/if}
				<span class="recast-sr-only">{resolvedBranding.alt} (opens in a new tab)</span>
			</a>
		{:else}
			<div class={`recast-branding ${resolvedBranding.className ?? ""}`} aria-hidden="true">
				<span class="recast-branding-mark">
					<img
						src={resolvedBranding.src}
						alt=""
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

	{#if mergedControls.bigPlay && !mediaError}
		<media-play-button class="recast-big-play">
			<span slot="play" class="recast-icon recast-icon-big">
				<Play class="size-7 translate-x-px" />
			</span>
			<span slot="pause" class="recast-icon recast-icon-big">
				<Pause class="size-7" />
			</span>
		</media-play-button>
	{/if}

	{#if mergedControls.playbackRate}
		<media-playback-rate-menu
			id={rateMenuId}
			class="recast-menu"
			rates={PLAYBACK_RATES.join(" ")}
			anchor="auto"
			hidden
		></media-playback-rate-menu>
	{/if}

	<media-control-bar class="recast-control-bar" noautohide={pinControls ? "" : undefined}>
		<!-- Two flush rows on a scrim: scrubber full-width along the top, transport
		     and utility controls split left / right on the row below. -->
		<div class="recast-pill">
			<div class="recast-scrubber-wrap">
				<media-time-range class="recast-scrubber">
					{#if thumbnails}
						<media-preview-thumbnail slot="preview" class="recast-thumb"></media-preview-thumbnail>
					{/if}
					<media-preview-time-display slot="preview" class="recast-preview-time"></media-preview-time-display>
				</media-time-range>

				{#if markers.length > 0}
					<div class="recast-marker-rail">
						{#each markers as marker (marker.id)}
							{@const markerTooltipId = `marker-${marker.id}`}
							{@const left = markerLeftPct(marker.time, duration)}
							<button
								type="button"
								class="recast-marker"
								style={`left:${left}%;--recast-marker-color:${markerColor(marker)};`}
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
									style={`left:${left}%;`}
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
					<media-play-button class="recast-btn">
						<span slot="play" class="recast-icon"><Play class="size-4 translate-x-[0.5px]" /></span>
						<span slot="pause" class="recast-icon"><Pause class="size-4" /></span>
					</media-play-button>

					{#if mergedControls.seek}
						<media-seek-backward-button class="recast-btn recast-btn-seek" seekoffset="10">
							<span slot="icon" class="recast-icon"><RotateCcw class="size-4" /></span>
						</media-seek-backward-button>

						<media-seek-forward-button class="recast-btn recast-btn-seek" seekoffset="10">
							<span slot="icon" class="recast-icon"><RotateCw class="size-4" /></span>
						</media-seek-forward-button>
					{/if}

					{#if mergedControls.time}
						<media-time-display class="recast-time" showduration></media-time-display>
					{/if}
				</div>

				<div class="recast-group recast-group-end">
					{#if mergedControls.volume}
						<button
							type="button"
							class="recast-btn"
							aria-label={muted || volume === 0 ? "Unmute" : "Mute"}
							onclick={toggleMute}
						>
							<span class="recast-icon">
								<VolumeIcon class="size-4" />
							</span>
						</button>
						<media-volume-range class="recast-volume"></media-volume-range>
					{/if}

					{#if mergedControls.playbackRate}
						<media-playback-rate-menu-button
							class="recast-btn recast-btn-text"
							invoketarget={rateMenuId}
						></media-playback-rate-menu-button>
					{/if}

					{#if hasCaptionTrack && mergedControls.captions}
						<!-- Toggles OUR styled overlay, not the native track (kept hidden
						     so the UA never paints its default boxes). -->
						<button
							type="button"
							class="recast-btn recast-caption-btn"
							aria-label="Captions"
							aria-pressed={captionsEnabled}
							onclick={() => (captionsEnabled = !captionsEnabled)}
						>
							<span class="recast-icon">
								<Captions class="size-4" />
							</span>
						</button>
					{/if}

					{#if mergedControls.pip}
						<media-pip-button class="recast-btn">
							<span slot="enter" class="recast-icon"><PictureInPicture class="size-4" /></span>
							<span slot="exit" class="recast-icon"><PictureInPicture2 class="size-4" /></span>
						</media-pip-button>
					{/if}

					{#if mergedControls.fullscreen}
						<media-fullscreen-button class="recast-btn">
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

	.recast-sr-only {
		position: absolute;
		width: 1px;
		height: 1px;
		overflow: hidden;
		clip-path: inset(50%);
		white-space: nowrap;
	}

	/* Styled caption overlay. Fills the media area (a size container so the
	   caption's `cqh` font tracks the player height) and never eats pointer
	   events. The active caption box is placed by `.recast-caption-slot`. */
	.recast-caption-layer {
		position: absolute;
		inset: 0;
		z-index: 2;
		container-type: size;
		pointer-events: none;
	}
	.recast-caption-slot {
		position: absolute;
		left: 0;
		right: 0;
		display: flex;
		padding: 0 6%;
	}
	@media (prefers-reduced-motion: reduce) {
		/* CaptionBox drops its own entrance/scale; this covers the branding and
		   control transitions. Opacity fades are kept — they aren't motion. */
		:global(.recast-player *) {
			transition-property: opacity, background-color, color !important;
			animation-duration: 0.01ms !important;
		}
	}

	.recast-error {
		position: absolute;
		inset: 0;
		z-index: 5;
		display: grid;
		place-content: center;
		justify-items: center;
		gap: 12px;
		padding: 24px;
		text-align: center;
		background: rgba(10, 10, 10, 0.82);
	}

	.recast-error-message {
		margin: 0;
		max-width: 34ch;
		color: rgba(255, 255, 255, 0.96);
		font-size: 14px;
		line-height: 1.45;
	}

	.recast-error-retry {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		min-height: 36px;
		padding: 0 14px;
		border: 1px solid rgba(255, 255, 255, 0.24);
		border-radius: 999px;
		background: rgba(255, 255, 255, 0.08);
		color: rgba(255, 255, 255, 0.96);
		font-size: 13px;
		font-weight: 500;
	}

	.recast-error-retry:hover {
		background: rgba(255, 255, 255, 0.16);
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
		background: #111111;
		color: #ffffff;
		box-shadow: 0 8px 20px rgba(0, 0, 0, 0.35);
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
