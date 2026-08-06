import type { CaptionStyle } from "@recast/captions";

export type RecastPlayerTrack = {
	src: string;
	kind: "subtitles" | "captions" | "chapters" | "descriptions" | "metadata";
	label?: string;
	srclang?: string;
	default?: boolean;
};

export type RecastPlayerControls = {
	bigPlay: boolean;
	seek: boolean;
	time: boolean;
	volume: boolean;
	playbackRate: boolean;
	captions: boolean;
	pip: boolean;
	fullscreen: boolean;
};

export type RecastPlayerBranding = {
	src?: string | null;
	alt?: string;
	name?: string;
	href?: string | null;
	width?: number;
	height?: number;
	className?: string;
};

export type RecastPlayerMarker = {
	id: string;
	time: number;
	label: string;
	kind?: "chapter" | "comment" | "highlight" | "cta";
	color?: string;
};

export type RecastPlayerActionEvent =
	| { type: "download"; src: string }
	| { type: "screenshot"; currentTime: number; dataUrl: string }
	| { type: "theater"; active: boolean }
	| { type: "marker-select"; marker: RecastPlayerMarker };

export type RecastPlayerState = {
	paused: boolean;
	ended: boolean;
	currentTime: number;
	duration: number;
	volume: number;
	muted: boolean;
	playbackRate: number;
	videoWidth: number;
	videoHeight: number;
	pictureInPicture: boolean;
	theaterMode: boolean;
};

/**
 * Engagement events fired by RecastPlayer. `progress` is throttled to
 * ~5% steps so a long video can't spam the parent with hundreds of calls.
 */
export type RecastPlayerEngagement =
	| { type: "view-start"; percent: 0 }
	| { type: "progress"; percent: number; currentTime: number }
	| { type: "ended"; percent: 100; currentTime: number };

export type RecastPlayerApi = {
	play: () => Promise<void>;
	pause: () => void;
	seek: (seconds: number) => void;
	setMuted: (next: boolean) => void;
	setVolume: (next: number) => void;
	setPlaybackRate: (next: number) => void;
	togglePlay: () => Promise<void>;
	setTheaterMode: (next: boolean) => void;
	enterFullscreen: () => Promise<void>;
	exitFullscreen: () => Promise<void>;
	enterPictureInPicture: () => Promise<void>;
	/** Downloads the current source, routing cross-origin URLs through a blob. */
	download: () => Promise<void>;
	/** PNG data URL of the current frame, or null if the media isn't CORS-readable. */
	captureScreenshot: () => string | null;
	/** Reloads the media element — the retry path out of an error state. */
	reload: () => void;
	getCurrentTime: () => number;
	getDuration: () => number;
	getState: () => RecastPlayerState;
	getVideoElement: () => HTMLVideoElement | null;
};

export type RecastPlayerProps = {
	src: string;
	poster?: string | null;
	thumbnails?: string | null;
	tracks?: RecastPlayerTrack[];
	/**
	 * Caption look for the styled overlay. Merged over the Loom default from
	 * @recast/captions. When a caption/subtitles `track` carries WebVTT word
	 * timestamps, the overlay highlights word-by-word; otherwise it shows the
	 * whole cue. Overrides the browser's default cue boxes.
	 */
	captionStyle?: Partial<CaptionStyle>;
	title?: string;
	autoplay?: boolean;
	preload?: "none" | "metadata" | "auto";
	/**
	 * Forces CORS mode. The source MUST send `Access-Control-Allow-Origin` or the
	 * media will not load at all; pass `null` for hosts that can't (screenshots
	 * and same-tab downloads stop working then).
	 */
	crossorigin?: "anonymous" | "use-credentials" | null;
	loop?: boolean;
	volume?: number;
	muted?: boolean;
	playbackRate?: number;
	currentTime?: number;
	paused?: boolean | null;
	markers?: RecastPlayerMarker[];
	controls?: Partial<RecastPlayerControls>;
	/** Media-chrome hotkeys plus its built-in `?` shortcuts dialog. */
	keyboardShortcuts?: boolean;
	aspectRatio?: number | string | null;
	/**
	 * Seconds of pointer inactivity before the control bar auto-hides during
	 * playback (media-chrome's `autohide`). Pass a negative value (e.g. `-1`)
	 * to keep the controls permanently visible — the right call for framed
	 * preview surfaces (the dashboard/desktop player dialogs) where the video
	 * may autoplay and would otherwise hide its controls before the viewer
	 * ever moves the pointer. Omitted → media-chrome's 2s default (immersive
	 * share page).
	 */
	autohide?: number | null;
	objectFit?: "contain" | "cover" | "fill" | "none" | "scale-down";
	ariaLabel?: string;
	className?: string;
	onengagement?: (event: RecastPlayerEngagement) => void;
	onstatechange?: (state: RecastPlayerState) => void;
	onaction?: (event: RecastPlayerActionEvent) => void;
	api?: RecastPlayerApi | null;
};
