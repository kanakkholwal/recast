/**
 * Pure time/text formatting for the public share page. Extracted from
 * `routes/share/[id]/+page.svelte` so the parsing/formatting is isolated from
 * the player wiring. (apps/web has no unit-test runner yet, so these are not
 * unit-tested — they're plain pure functions verified by svelte-check.)
 */

/** A parsed segment of comment text — plain text, a clickable timestamp, or a mention. */
export type CommentSegment =
	| { kind: "text"; text: string }
	| { kind: "timestamp"; seconds: number; raw: string }
	| { kind: "mention"; name: string };

/**
 * Parse a `?t=` deep-link value into seconds. Accepts a bare seconds count
 * (`90`) or a unit string (`1h2m3s`); missing units default to 0.
 */
export function parseTimeParam(raw: string | null): number {
	if (!raw) return 0;
	const t = raw.trim().toLowerCase();
	if (/^\d+$/.test(t)) return Number(t);
	let total = 0;
	const h = t.match(/(\d+)h/);
	const m = t.match(/(\d+)m/);
	const s = t.match(/(\d+)s/);
	if (h) total += Number(h[1]) * 3600;
	if (m) total += Number(m[1]) * 60;
	if (s) total += Number(s[1]);
	return total;
}

/** Parse an `mm:ss` or `hh:mm:ss` token into seconds (0 on malformed input). */
export function parseTimeToken(s: string): number {
	const parts = s.split(":").map((p) => Number(p));
	if (parts.some(Number.isNaN)) return 0;
	if (parts.length === 2) return parts[0] * 60 + parts[1];
	if (parts.length === 3) return parts[0] * 3600 + parts[1] * 60 + parts[2];
	return 0;
}

/**
 * Split comment text into segments, linkifying `[mm:ss]`/`mm:ss` timestamps and
 * `@mentions`.
 */
export function parseCommentText(text: string): CommentSegment[] {
	const re = /\[(\d{1,2}(?::\d{2}){1,2})\]|\b(\d{1,2}:\d{2}(?::\d{2})?)\b|@([A-Za-z][\w]{0,31})/g;
	const out: CommentSegment[] = [];
	let lastIdx = 0;
	for (let m = re.exec(text); m !== null; m = re.exec(text)) {
		if (m.index > lastIdx) out.push({ kind: "text", text: text.slice(lastIdx, m.index) });
		if (m[1] !== undefined)
			out.push({ kind: "timestamp", seconds: parseTimeToken(m[1]), raw: m[1] });
		else if (m[2] !== undefined)
			out.push({ kind: "timestamp", seconds: parseTimeToken(m[2]), raw: m[2] });
		else if (m[3] !== undefined) out.push({ kind: "mention", name: m[3] });
		lastIdx = m.index + m[0].length;
	}
	if (lastIdx < text.length) out.push({ kind: "text", text: text.slice(lastIdx) });
	return out;
}

/** `m:ss` clock for the player readout. */
export function formatTime(sec: number): string {
	const m = Math.floor(sec / 60);
	const s = Math.floor(sec % 60);
	return `${m}:${String(s).padStart(2, "0")}`;
}

/** Compact unit string for `?t=` (`1h2m3s`); empty at 0. */
export function compactTime(sec: number): string {
	const s = Math.max(0, Math.floor(sec));
	if (s === 0) return "";
	const h = Math.floor(s / 3600);
	const m = Math.floor((s % 3600) / 60);
	const r = s % 60;
	let out = "";
	if (h) out += `${h}h`;
	if (m) out += `${m}m`;
	if (r || !out) out += `${r}s`;
	return out;
}

/** Avatar fallback initials from a name or email. */
export function initials(
	name: string | null | undefined,
	email: string | null | undefined,
): string {
	const src = (name ?? email ?? "?").trim();
	if (!src) return "?";
	const parts = src.split(/\s+/).filter(Boolean);
	if (parts.length >= 2) return (parts[0][0] + parts[1][0]).toUpperCase();
	return src.slice(0, 2).toUpperCase();
}

/** Deterministic hue (0..359) from a seed string, for per-commenter colour. */
export function commentHue(seed: string): number {
	let h = 0;
	for (let i = 0; i < seed.length; i++) h = (h * 31 + seed.charCodeAt(i)) >>> 0;
	return h % 360;
}
