/**
 * @recast/player — media-chrome-backed video player for the Recast suite.
 * HLS via `hls-video-element`, native MP4 otherwise; same package in
 * `apps/web` and `apps/desktop`.
 *
 * Consumers MUST also import the stylesheet once at the app entry:
 *   `import "@recast/player/styles.css";`
 */

export { default as RecastPlayer } from "./RecastPlayer.svelte";
export type {
	RecastPlayerProps,
	RecastPlayerEngagement,
	RecastPlayerApi,
	RecastPlayerActionEvent,
	RecastPlayerControls,
	RecastPlayerMarker,
	RecastPlayerState,
	RecastPlayerTrack,
} from "./types";
