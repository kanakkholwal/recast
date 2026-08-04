/**
 * Pure shell state helpers for `<Editor />`: layout persistence and the
 * loop-vs-pause decision at end of clip. Split out so they're unit-testable
 * without mounting the editor.
 */

export interface ShellLayout {
	sidebar: boolean;
	timeline: boolean;
}

export const LAYOUT_KEY = "recast-editor-layout";
export const SIDEBAR_WIDTH_KEY = "recast-editor-sidebar-width";

export const SIDEBAR_MIN = 352;
export const SIDEBAR_MAX = 600;
export const SIDEBAR_DEFAULT = 384;

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

export function clampSidebarWidth(w: number): number {
	if (!Number.isFinite(w)) return SIDEBAR_DEFAULT;
	return Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, Math.round(w)));
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

/** Panels the host offers, filtered to those the services can actually serve. */
export function visiblePanels<T extends string>(
	requested: readonly T[],
	available: { [K in T]?: boolean },
): T[] {
	return requested.filter((p) => available[p] !== false);
}
