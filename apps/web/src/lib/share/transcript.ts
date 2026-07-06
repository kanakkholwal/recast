/**
 * Interactive-transcript helpers for the share page. The captions VTT is
 * already loaded by the player as a `<track>`, so we read cues straight off the
 * live `TextTrack` (no second network trip, no cross-origin fetch of the signed
 * R2 URL). These helpers stay pure/DOM-light so the parsing + active-line math
 * is isolated from the player wiring. (apps/web has no unit runner — verified by
 * svelte-check.)
 */

/** One transcript line: a caption cue reduced to plain text + its time span. */
export type TranscriptCue = {
	id: string;
	start: number;
	end: number;
	text: string;
};

/**
 * Flatten a live `TextTrack`'s cues into transcript lines. Strips VTT inline
 * markup (`<v Speaker>`, `<00:00:01.000>` karaoke tags, `<b>` …) and collapses
 * whitespace. Returns `[]` when the cues haven't parsed yet — callers poll.
 */
export function readCuesFromTrack(track: TextTrack): TranscriptCue[] {
	const cues = track.cues;
	if (!cues || cues.length === 0) return [];
	const out: TranscriptCue[] = [];
	for (let i = 0; i < cues.length; i++) {
		const cue = cues[i] as VTTCue;
		const text = (cue.text ?? "")
			.replace(/<[^>]+>/g, "")
			.replace(/\s+/g, " ")
			.trim();
		if (!text) continue;
		out.push({
			id: cue.id || `cue-${i}`,
			start: cue.startTime,
			end: cue.endTime,
			text,
		});
	}
	return out;
}

/**
 * Index of the cue that should read as "now" at time `t` — the last cue whose
 * start has passed. Returns -1 before the first cue. Cues are assumed sorted by
 * start (VTT guarantees this). A gap between cues keeps the previous line lit,
 * which reads better than flickering to nothing.
 */
export function activeCueIndex(cues: TranscriptCue[], t: number): number {
	let idx = -1;
	for (let i = 0; i < cues.length; i++) {
		if (cues[i]!.start <= t) idx = i;
		else break;
	}
	return idx;
}

/** Case-insensitive substring filter for the transcript search box. */
export function filterCues(cues: TranscriptCue[], query: string): TranscriptCue[] {
	const q = query.trim().toLowerCase();
	if (!q) return cues;
	return cues.filter((c) => c.text.toLowerCase().includes(q));
}
