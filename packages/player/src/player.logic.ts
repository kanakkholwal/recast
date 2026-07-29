import type { RecastPlayerEngagement } from "./types";

/** Keys `<media-controller>` claims for itself (media-chrome `ButtonPressedKeys`). */
export const MEDIA_CHROME_HOTKEYS = [
	"ArrowLeft",
	"ArrowRight",
	"ArrowUp",
	"ArrowDown",
	"Enter",
	" ",
	"f",
	"m",
	"k",
	"c",
	"l",
	"j",
	">",
	"<",
	"p",
] as const;

/** Keys RecastPlayer handles itself. Must stay disjoint from the set above. */
export const RECAST_OWNED_KEYS = ["Home", "End"] as const;

export function conflictingHotkeys(
	owned: readonly string[] = RECAST_OWNED_KEYS,
	library: readonly string[] = MEDIA_CHROME_HOTKEYS,
) {
	const claimed = new Set(library.map((key) => key.toLowerCase()));
	return owned.filter((key) => claimed.has(key.toLowerCase()));
}

export function markerLeftPct(time: number, duration: number) {
	if (!duration || !Number.isFinite(duration) || duration <= 0) return 0;
	if (!Number.isFinite(time)) return 0;
	return Math.max(0, Math.min(100, (time / duration) * 100));
}

export type VolumeLevel = "muted" | "low" | "medium" | "high";

export function volumeLevel(volume: number, muted: boolean): VolumeLevel {
	if (muted || volume === 0) return "muted";
	if (volume < 0.34) return "low";
	if (volume < 0.67) return "medium";
	return "high";
}

export type DownloadPlan = {
	strategy: "anchor" | "fetch-blob";
	filename: string;
};

function safeFilename(title: string) {
	const cleaned = title.replace(/[\\/:*?"<>|]+/g, "-").trim();
	return cleaned ? `${cleaned}.mp4` : "video.mp4";
}

/**
 * A cross-origin `<a download>` is ignored by the browser and navigates
 * instead, so signed CDN URLs have to go through a blob.
 */
export function resolveDownloadPlan(src: string, title: string, pageOrigin: string): DownloadPlan {
	const filename = safeFilename(title);
	let sameOrigin = true;
	try {
		sameOrigin = new URL(src, pageOrigin).origin === new URL(pageOrigin).origin;
	} catch {
		sameOrigin = true;
	}
	return { strategy: sameOrigin ? "anchor" : "fetch-blob", filename };
}

/**
 * Throttles view/progress/ended reporting to ~5% steps. One tracker per
 * playback session; call `reset()` when the source changes.
 */
export class EngagementTracker {
	#started = false;
	#lastReportedPct = 0;

	onPlay(): RecastPlayerEngagement | null {
		if (this.#started) return null;
		this.#started = true;
		return { type: "view-start", percent: 0 };
	}

	onTimeUpdate(currentTime: number, duration: number): RecastPlayerEngagement | null {
		if (!duration || !Number.isFinite(duration)) return null;
		const pct = Math.min(100, Math.round((currentTime / duration) * 100));
		// Absolute distance, so scrubbing backwards re-arms reporting instead of
		// going silent until playback climbs past the old high-water mark.
		if (Math.abs(pct - this.#lastReportedPct) < 5) return null;
		this.#lastReportedPct = pct;
		return { type: "progress", percent: pct, currentTime };
	}

	onEnded(currentTime: number): RecastPlayerEngagement {
		return { type: "ended", percent: 100, currentTime };
	}

	reset() {
		this.#started = false;
		this.#lastReportedPct = 0;
	}
}
