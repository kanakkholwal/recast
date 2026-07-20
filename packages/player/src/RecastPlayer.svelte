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
	  VolumeX as VolumeMuted
	} from "@recast/icons";
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

	// Minimal by default: play, time, then speed / volume / fullscreen. The +-10s
	// jog buttons and PiP are opt-in — the scrubber covers the same intent and a
	// six-button row was reading as a toolbar bolted onto the video.
	const DEFAULT_CONTROLS: RecastPlayerControls = {
		bigPlay: true,
		seek: false,
		time: true,
		volume: true,
		playbackRate: true,
		captions: false,
		pip: false,
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
	const playerLabel = $derived(ariaLabel || title || "Video player");

	// Mute is a plain button, not `media-mute-button`: media-chrome swaps its
	// off/low/medium/high icons via shadow-DOM rules keyed on a
	// `mediavolumelevel` attribute the controller does not propagate reliably
	// here. Deriving from our own state is deterministic.
	const VolumeIcon = $derived(
		muted || volume === 0
			? VolumeMuted
			: volume < 0.34
				? VolumeLow
				: volume < 0.67
					? VolumeMedium
					: VolumeHigh,
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
	// active cue's words on every cue change. Track + cues land async after mount,
	// so poll until present.
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
			if (!found) return false;
			track = found;
			track.mode = "hidden";
			track.addEventListener("cuechange", readActive);
			readActive();
			return true;
		};
		let iv: ReturnType<typeof setInterval> | null = null;
		if (!attach()) {
			let tries = 0;
			iv = setInterval(() => {
				if (attach() || ++tries > 25) {
					if (iv) clearInterval(iv);
					iv = null;
				}
			}, 200);
		}
		return () => {
			if (iv) clearInterval(iv);
			track?.removeEventListener("cuechange", readActive);
			cueWords = [];
		};
	});
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
				toggleMute();
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

	{#if captionView}
		<!-- `noautohide`: media-chrome fades every slotted controller child on
		     inactivity EXCEPT slot=media/poster, role=dialog, or [noautohide].
		     Without it the captions would fade out with the control bar; captions
		     must stay until toggled off. -->
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
		<!-- Two flush rows on a scrim: scrubber full-width along the top, transport
		     and utility controls split left / right on the row below. -->
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
						<button
							type="button"
							class="recast-btn"
							aria-label={muted || volume === 0 ? "Unmute" : "Mute"}
							aria-pressed={muted}
							onclick={toggleMute}
						>
							<span class="recast-icon">
								<VolumeIcon class="size-4" />
							</span>
						</button>
						<media-volume-range class="recast-volume" aria-label="Volume"></media-volume-range>
					{/if}

					{#if mergedControls.playbackRate}
						<media-playback-rate-button
							class="recast-btn recast-btn-text"
							rates="0.25 0.5 0.75 1 1.25 1.5 1.75 2"
							aria-label="Playback speed"
						></media-playback-rate-button>
					{/if}

					{#if hasCaptionTrack && showMenu}
						<!-- Toggles OUR styled overlay, not the native track (kept hidden
						     so the UA never paints its default boxes). -->
						<button
							type="button"
							class="recast-btn recast-caption-btn"
							aria-label={captionsEnabled ? "Hide captions" : "Show captions"}
							aria-pressed={captionsEnabled}
							onclick={() => (captionsEnabled = !captionsEnabled)}
						>
							<span class="recast-icon">
								<Captions class="size-4" />
							</span>
						</button>
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
		/* The player had no reduced-motion handling; CaptionBox drops its own
		   entrance/scale, this covers the branding/control transitions too. */
		:global(.recast-player *) {
			transition-duration: 0.01ms !important;
			animation-duration: 0.01ms !important;
		}
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
