/**
 * @recast/player — media-chrome-backed video player for the Recast suite.
 *
 *   - HLS adaptive streaming via the companion `hls-video-element` (falls
 *     back to native MP4 for non-`.m3u8` sources).
 *   - Slot-based composition so editor surfaces (timestamped comments,
 *     transcripts, AI cut markers) can wrap or extend the player without
 *     fighting a pre-built layout.
 *   - Framework-agnostic web components under the hood — same package
 *     works in `apps/web` (SvelteKit) and `apps/desktop` (Tauri).
 *
 * Consumers MUST also import the stylesheet once at the app entry:
 *   `import "@recast/player/styles.css";`
 */

export { default as RecastPlayer } from "./RecastPlayer.svelte";
export type {
	RecastPlayerProps,
	RecastPlayerEngagement,
	RecastPlayerApi,
} from "./types";
