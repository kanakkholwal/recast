/**
 * The event taxonomy — the single source of truth for every product-analytics
 * event Recast emits, shared by both the web app and the desktop app so the
 * names never drift between surfaces.
 *
 * Conventions:
 *   - `snake_case`, `object_action`, past tense for completed actions
 *     (`export_completed`, not `complete_export`).
 *   - Properties are flat, `snake_case`, and carry **no PII** — no filenames,
 *     absolute paths, share slugs, emails, or hostnames. IDs only when they're
 *     non-identifying (a visibility enum, a codec name, a duration).
 *   - Add an event by extending `ANALYTICS_EVENTS`; add a typed prop shape in
 *     `EventPropMap` if call sites should be constrained.
 *
 * Global super-properties (`app_version`, `os`, `source`, `user_plan`,
 * `user_type`) are registered once at init by each app's analytics client and
 * merged into every event — do NOT pass them per-call.
 */

export const ANALYTICS_EVENTS = [
	"app_opened",
	"recording_started",
	"recording_stopped",
	"recording_paused",
	"export_started",
	"export_completed",
	"export_failed",
	"recast_uploaded",
	"share_created",
	"share_viewed",
	"share_player_error",
	"share_signup_cta_click",
	// Share funnel: play-rate, drop-off, and per-placement CTA click-through.
	"share_play_started",
	"share_watch_depth",
	"share_cta_impression",
	"editor_opened",
	"cloud_connected",
	"sign_in",
	"sign_out",
	"consent_granted",
	"consent_revoked",
	// Experimental WebCodecs preview engine — the signal that gates default-on:
	// init-success / fallback-rate / decode-fps, dimensioned by OS (PostHog's
	// auto `$os`) + resolution. Drop these once the engine graduates.
	"webcodecs_preview_init",
	"webcodecs_preview_fallback",
	"webcodecs_preview_perf",
] as const;

export type AnalyticsEvent = (typeof ANALYTICS_EVENTS)[number];

/**
 * Optional typed prop shapes for the events worth constraining. Call sites can
 * import these to get autocomplete + a compile error if they pass the wrong
 * field — but `capture` stays permissive for events not listed here.
 */
export interface EventPropMap {
	recording_stopped: {
		duration_ms?: number;
		source_kind?: string;
		has_camera?: boolean;
		has_mic?: boolean;
		has_system_audio?: boolean;
	};
	export_completed: {
		format?: string;
		duration_ms?: number;
		output_bytes?: number;
		encoder?: string;
	};
	export_failed: {
		reason?: string;
		encoder?: string;
	};
	share_created: {
		visibility: "private" | "workspace" | "selected" | "public";
		has_password?: boolean;
		has_expiry?: boolean;
		watermark?: boolean;
	};
	share_viewed: {
		visibility?: string;
		watch_pct?: number;
		completed?: boolean;
		/** The anonymous `shareView` session id, so PostHog reconciles with the
		 * first-party watch-metrics table. Viewers are NOT identified. */
		share_session_id?: string;
	};
	share_player_error: {
		/** Error class name only (e.g. "TypeError") — never the raw message (no paths/PII). */
		reason?: string;
	};
	share_signup_cta_click: {
		/** Which acquisition surface converted the viewer. */
		placement?:
			| "header"
			| "end-card"
			| "watermark"
			| "mid-watch"
			| "positioning-chip";
		visibility?: string;
	};
	share_play_started: {
		visibility?: string;
		/** Anonymous shareView session id (not identifying). */
		share_session_id?: string;
	};
	share_watch_depth: {
		/** Milestone reached: 25 / 50 / 75 / 100. */
		pct?: number;
		visibility?: string;
	};
	share_cta_impression: {
		/** Which acquisition surface was actually shown, to pair with clicks. */
		placement?: "end-card" | "mid-watch" | "positioning-chip";
		visibility?: string;
	};
	recast_uploaded: {
		size_bytes?: number;
		width?: number;
		height?: number;
		fps?: number;
	};
	webcodecs_preview_init: {
		width?: number;
		height?: number;
		fps?: number;
		/** Coarse bucket (e.g. "1080p", "4k") for easy cohorting. */
		resolution?: string;
		/** Ingestion strategy chosen for this source. */
		ingestion?: "whole" | "progressive";
	};
	webcodecs_preview_fallback: {
		/** Classified reason — never the raw error (no paths/PII). */
		reason?: string;
	};
	webcodecs_preview_perf: {
		/** Decoded frames/sec, averaged over the playback windows of this source. */
		avg_fps?: number;
		min_fps?: number;
		/** Worst frame lateness vs the playback clock, ms. */
		max_late_ms?: number;
		width?: number;
		height?: number;
		/** Source media fps, for context against avg_fps. */
		fps?: number;
		resolution?: string;
	};
}
