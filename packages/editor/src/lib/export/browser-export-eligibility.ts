/**
 * Gate for the browser export path: which projects it can render today, and which
 * fall back to the Rust compositor. Kept dependency-free (type-only imports) so
 * it's a pure, testable predicate — the orchestrator (browser-export.ts) and the
 * editor page both consult it.
 */

import type { EditorStore } from "../../stores/editor-store.svelte";

/** Effective export frame rate — the rate the renderer actually encodes at: the
 *  picker value, else the GIF setting, else the source rate. Shared so the editor
 *  and the eligibility gate agree on the number. */
export function resolveExportFps(store: EditorStore): number {
	const srcFps = store.metadata?.fps;
	if (store.exportFormat === "gif") {
		const g = store.gifSettings?.fps;
		return g && g > 0 ? g : (srcFps ?? 15);
	}
	return store.exportFps && store.exportFps > 0 ? store.exportFps : (srcFps ?? 30);
}

// Above this pixel throughput the browser pipeline (decode + WebGL + WebCodecs on
// one GPU, alongside the live preview) over-subscribes the GPU and loses its
// context mid-render. Route such sources to the Rust compositor, which handles
// them reliably. 1080p60 is the highest tier verified in the browser; 1080p120
// and 4K land here. Raise this as the browser path proves out on more hardware.
const SAFE_EXPORT_THROUGHPUT = 1920 * 1080 * 60;

/** Why a project can't use the browser export path (so the caller falls back to
 *  the Rust compositor), or null when it's eligible. Every effect renders in the
 *  browser now — all annotation kinds, GIF, burned captions — so the only gate is
 *  raw throughput: a source heavy enough to over-subscribe the GPU goes to Rust. */
export function browserExportBlockedReason(store: EditorStore): string | null {
	const m = store.metadata;
	if (m?.width && m?.height) {
		const fps = resolveExportFps(store);
		if (m.width * m.height * fps > SAFE_EXPORT_THROUGHPUT) {
			return `heavy source (${m.width}×${m.height} @ ${Math.round(fps)}fps)`;
		}
	}
	return null;
}
