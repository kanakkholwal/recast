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
	position?: "top-left";
};

export type RecastPlayerChapter = {
	id?: string;
	label: string;
	startTime: number;
	endTime?: number | null;
};

export type RecastPlayerMarker = {
	id: string;
	time: number;
	label: string;
	kind?: "chapter" | "comment" | "highlight" | "cta";
	color?: string;
};

export type RecastPlayerUtilityAction =
	| { id: "share"; label?: string }
	| { id: "download"; label?: string }
	| { id: "screenshot"; label?: string }
	| { id: "theater"; label?: string }
	| { id: "chapters"; label?: string }
	| { id: "shortcuts"; label?: string }
	| { id: "settings"; label?: string }
	| { id: "pip"; label?: string }
	| { id: "custom"; label: string; actionId: string };

export type RecastPlayerFeatures = {
	settingsMenu: boolean;
	chaptersMenu: boolean;
	theaterMode: boolean;
	miniPlayer: boolean;
	share: boolean;
	download: boolean;
	screenshot: boolean;
	keyboardShortcuts: boolean;
	markers: boolean;
};

export type RecastPlayerActionEvent =
	| { type: "share"; currentTime: number }
	| { type: "download"; src: string }
	| { type: "screenshot"; currentTime: number; dataUrl: string }
	| { type: "theater"; active: boolean }
	| { type: "chapter-select"; chapter: RecastPlayerChapter }
	| { type: "marker-select"; marker: RecastPlayerMarker }
	| { type: "custom"; actionId: string; currentTime: number };

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
	openSettings: () => void;
	closeSettings: () => void;
	enterFullscreen: () => Promise<void>;
	exitFullscreen: () => Promise<void>;
	enterPictureInPicture: () => Promise<void>;
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
	crossorigin?: "anonymous" | "use-credentials" | null;
	loop?: boolean;
	volume?: number;
	muted?: boolean;
	playbackRate?: number;
	currentTime?: number;
	paused?: boolean | null;
	chapters?: RecastPlayerChapter[];
	markers?: RecastPlayerMarker[];
	utilityActions?: RecastPlayerUtilityAction[];
	features?: Partial<RecastPlayerFeatures>;
	showMenu?: boolean;
	controls?: Partial<RecastPlayerControls>;
	branding?: RecastPlayerBranding | null;
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
