/**
 * Pure shell state helpers for `<Editor />`: layout persistence and the
 * end-of-clip transport decision. Split out so they're unit-testable without
 * mounting the editor. Panel bounds live in `lib/editor/panel-size`.
 */

export interface ShellLayout {
	sidebar: boolean;
	timeline: boolean;
}

export const LAYOUT_KEY = "recast-editor-layout";
export const SIDEBAR_WIDTH_KEY = "recast-editor-sidebar-width";
export const TIMELINE_HEIGHT_KEY = "recast-editor-timeline-height";

export const DEFAULT_LAYOUT: ShellLayout = { sidebar: true, timeline: true };

/** Missing or malformed persisted layout falls back to everything visible. */
export function parseLayout(raw: string | null): ShellLayout {
	if (!raw) return { ...DEFAULT_LAYOUT };
	try {
		const v = JSON.parse(raw) as Partial<ShellLayout>;
		return {
			sidebar: typeof v.sidebar === "boolean" ? v.sidebar : DEFAULT_LAYOUT.sidebar,
			timeline: typeof v.timeline === "boolean" ? v.timeline : DEFAULT_LAYOUT.timeline,
		};
	} catch {
		return { ...DEFAULT_LAYOUT };
	}
}

/**
 * What the transport should do when playback reaches the end of the trim.
 * `loop` rewinds to `trimStart` and keeps playing; `pause` stops there.
 */
export function endOfClipAction(loopEnabled: boolean): "loop" | "pause" {
	return loopEnabled ? "loop" : "pause";
}

/**
 * Whether the host's time echo should be ignored. When the WebCodecs engine owns
 * the picture clock the `<video>` element free-runs through the un-cut source,
 * so feeding its time back snaps playback across every cut.
 */
export function shouldEchoElementTime(webcodecsActive: boolean): boolean {
	return !webcodecsActive;
}

/** Read a persisted number, falling back when the key is absent or junk. */
export function readStoredNumber(raw: string | null, fallback: number): number {
	const n = Number(raw);
	return raw !== null && Number.isFinite(n) && n > 0 ? n : fallback;
}
