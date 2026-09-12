/**
 * Analysis service: "understand this recording and suggest edits" orchestration
 * (currently smart auto-zoom). Owns NO UI state: does IPC + computation +
 * persistence and returns a structured outcome; the caller surfaces it. This is
 * the surface a future MCP "auto-edit" tool calls. See ./README.md for layering.
 */

import { applyAutoZooms } from "@recast/editor/lib/zoom/auto-apply";
import type { EditorStore } from "@recast/editor/stores/editor-store.svelte";
import { suggestZoomRegions } from "$lib/ipc";

export interface AutoZoomOutcome {
	/** Number of focus regions actually placed. */
	applied: number;
	reason: "applied" | "empty" | "bad-bounds";
}

/**
 * Detect focus candidates from a cursor track and place focus regions, under a
 * single coalesced undo entry. Latches `autoZoomApplied` before it can fail, so a
 * crash cannot re-run it and double the regions. Persisting is the caller's job,
 * as is toasting and guarding concurrent runs.
 */
export async function generateAutoZoom(
	store: EditorStore,
	cursorPath: string,
): Promise<AutoZoomOutcome> {
	const suggestions = await suggestZoomRegions(cursorPath);
	const dur = store.metadata?.duration ?? 0;
	const w = store.metadata?.width ?? 0;
	const h = store.metadata?.height ?? 0;
	const bounds = {
		start: store.inPoint,
		end: store.outPoint > 0 ? store.outPoint : dur,
	};
	if (bounds.end <= bounds.start) {
		// Nothing to place, but latch the flag so we don't retry every reopen.
		store.autoZoomApplied = true;
		return { applied: 0, reason: "bad-bounds" };
	}

	store.pushUndoState();
	const result = applyAutoZooms(store, suggestions, bounds, w, h);
	// Latch before anything can fail, so a crash cannot re-run this and double the regions on reopen.
	store.autoZoomApplied = true;

	return {
		applied: result.applied,
		reason: result.applied > 0 ? "applied" : "empty",
	};
}
