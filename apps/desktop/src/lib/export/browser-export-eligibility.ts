/**
 * Gate for the browser export path: which projects it can render today, and which
 * fall back to the Rust compositor. Kept dependency-free (type-only imports) so
 * it's a pure, testable predicate — the orchestrator (browser-export.ts) and the
 * editor page both consult it.
 */

import type { EditorStore } from "$lib/stores/editor-store.svelte";

/** Mirror of `buildCaptionExport(...).burnCaptions` — inlined so the gate carries
 *  no heavy imports. Keep in lockstep with services/export.ts. */
function willBurnCaptions(store: EditorStore): boolean {
	const t = store.transcript;
	return !!t && t.segments.length > 0 && store.captionExport.burnIn && store.exportFormat !== "gif";
}

/** Why a project can't use the browser export path yet (so the caller falls back
 *  to the Rust compositor), or null when it's eligible. Painted + text
 *  annotations now render in the browser (text rasterized to an image); only
 *  blur still needs the Rust path (P1b framebuffer pass), as do burned captions.
 *  GIF keeps its 2-pass palette on the Rust side. */
export function browserExportBlockedReason(store: EditorStore): string | null {
	if (store.exportFormat === "gif") return "gif";
	if (willBurnCaptions(store)) return "burned captions";
	if (
		!store.annotationsGloballyHidden &&
		store.annotations.some((a) => !a.hidden && a.kind.kind === "blur")
	) {
		return "blur annotations";
	}
	return null;
}
