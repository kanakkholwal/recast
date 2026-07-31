/**
 * Pure helpers for the editor route: layout persistence parsing and the
 * repeated path-basename. The export state machine and audio-sync effects stay
 * in the component; time formatters live in $lib/format/time.
 */

export interface EditorLayout {
	sidebar: boolean;
	timeline: boolean;
}

/**
 * Parse the persisted sidebar/timeline layout from its raw localStorage string.
 * Missing, malformed, or non-boolean fields fall back to visible. The caller
 * keeps the `localStorage.getItem` read inline (browser-only).
 */
export function parseLayout(raw: string | null): EditorLayout {
	const fallback: EditorLayout = { sidebar: true, timeline: true };
	if (!raw) return fallback;
	try {
		const parsed = JSON.parse(raw) as Partial<EditorLayout>;
		return {
			sidebar: typeof parsed?.sidebar === "boolean" ? parsed.sidebar : true,
			timeline: typeof parsed?.timeline === "boolean" ? parsed.timeline : true,
		};
	} catch {
		return fallback;
	}
}

/** Rotating status messages shown below the progress ring during encode. */
export const ENCODE_MESSAGES = [
	"Crunching frames",
	"Encoding pixels",
	"Weaving the timeline",
	"Tuning the colours",
	"Squeezing the bitrate",
	"Polishing every frame",
];

/** Final path segment across both `/` and `\` separators. */
export function basename(path: string): string | undefined {
	return path.split(/[\\/]/).pop();
}
