/**
 * Engagement events fired by RecastPlayer. `progress` is throttled to
 * ~5% steps so a long video can't spam the parent with hundreds of calls.
 */
export type RecastPlayerEngagement =
	| { type: "view-start"; percent: 0 }
	| { type: "progress"; percent: number; currentTime: number }
	| { type: "ended"; percent: 100; currentTime: number };

/**
 * Imperative handle the player exposes back to the parent. Mounted onto
 * `bind:api` so editor surfaces can drive playback from any function —
 * transcript-click → `seek(cue.startTime)`, AI cut marker → `pause()` etc.
 *
 * `videoEl` is the underlying `<video>` / `<hls-video>` element so callers
 * can drop down to native APIs (frame-stepping with `requestVideoFrameCallback`,
 * `captureStream()` for thumbnail generation, etc.) when needed.
 */
export type RecastPlayerApi = {
	play: () => Promise<void>;
	pause: () => void;
	seek: (seconds: number) => void;
	getCurrentTime: () => number;
	getDuration: () => number;
	getVideoElement: () => HTMLVideoElement | null;
};

export type RecastPlayerProps = {
	/** Absolute video URL. `.m3u8` triggers HLS via hls-video-element. */
	src: string;
	/** Preview frame shown before play. */
	poster?: string | null;
	/** Sprite VTT for hover-scrub thumbnails (Cloudinary can generate). */
	thumbnails?: string | null;
	/** Display title surfaced to accessibility tooling. */
	title?: string;
	/** Autoplay when the player mounts (muted policy still applies). */
	autoplay?: boolean;
	/** Default volume 0–1. */
	volume?: number;
	/** Show the captions/settings menu. Defaults to true. */
	showMenu?: boolean;
	/** Optional aspect ratio class (e.g. "aspect-video"). */
	className?: string;
	/** Engagement reporter — fired from view-start → progress → ended. */
	onengagement?: (event: RecastPlayerEngagement) => void;
	/** Bindable imperative handle (Svelte 5 `$bindable`). */
	api?: RecastPlayerApi | null;
};
