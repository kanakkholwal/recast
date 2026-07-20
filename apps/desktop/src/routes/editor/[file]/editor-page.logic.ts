/**
 * Pure helpers for the editor route: layout persistence parsing, export ETA/
 * elapsed formatting, and the repeated path-basename. The export state machine
 * and audio-sync effects stay in the component.
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
			sidebar: typeof parsed?.sidebar === 'boolean' ? parsed.sidebar : true,
			timeline: typeof parsed?.timeline === 'boolean' ? parsed.timeline : true,
		};
	} catch {
		return fallback;
	}
}

/** Rotating status messages shown below the progress ring during encode. */
export const ENCODE_MESSAGES = [
	'Crunching frames',
	'Encoding pixels',
	'Weaving the timeline',
	'Tuning the colours',
	'Squeezing the bitrate',
	'Polishing every frame',
];

/** Compact elapsed time: `45s`, or `1m 05s` past a minute. */
export function formatElapsed(ms: number): string {
	const s = Math.floor(ms / 1000);
	if (s < 60) return `${s}s`;
	return `${Math.floor(s / 60)}m ${s % 60}s`;
}

/**
 * Export ETA from elapsed × (1 − pct) / pct. Null until it's meaningful: no
 * real progress yet, finalising, below 10 %, or under 250 ms elapsed.
 */
export function exportEtaMs(args: {
	hasProgress: boolean;
	finalizing: boolean;
	progress: number;
	now: number;
	startedAt: number;
}): number | null {
	if (!args.hasProgress || args.finalizing) return null;
	const pct = args.progress;
	if (pct < 10) return null;
	const elapsed = args.now - args.startedAt;
	if (elapsed < 250) return null;
	return (elapsed * (100 - pct)) / pct;
}

/** Final path segment across both `/` and `\` separators. */
export function basename(path: string): string | undefined {
	return path.split(/[\\/]/).pop();
}
