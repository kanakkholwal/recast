/**
 * Gate for the browser export path: which projects it can render today, and which
 * fall back to the Rust compositor. Kept dependency-free (type-only imports) so
 * it's a pure, testable predicate — the orchestrator (browser-export.ts) and the
 * editor page both consult it.
 */

import type { EditorStore } from "$lib/stores/editor-store.svelte";

/** Why a project can't use the browser export path (so the caller falls back to
 *  the Rust compositor), or null when it's eligible. Every effect now renders in
 *  the browser — all annotation kinds (painted/text/blur), GIF (browser
 *  composites, Rust runs only the palette), and burned captions — so nothing
 *  gates. Kept as a hook: the fallback-on-failure path still consults it, and a
 *  future effect that needs Rust would return its reason here. */
export function browserExportBlockedReason(_store: EditorStore): string | null {
	return null;
}
