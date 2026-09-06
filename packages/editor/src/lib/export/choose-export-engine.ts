/**
 * Export-engine resolver: the single decision for whether an export runs through
 * the wasm engine (the same compositor the preview draws with, so it is WYSIWYG
 * by construction) or the legacy Rust/FFmpeg one. Pure and dependency-free so it
 * unit-tests without the store/DOM.
 *
 * The engine is the default. Rust is reached by an automatic fallback
 * (capability, feature gate, throughput) or by the legacy setting.
 */

export type ExportEngine = "browser" | "rust";

export interface ExportEngineInputs {
	/** Kill switch for the whole path. True in the app; false only in tests. */
	masterEnabled: boolean;
	/** The "Use the legacy exporter" setting: the way back for a machine whose
	 *  WebView encodes badly, now that the engine is the default. */
	forceLegacy?: boolean;
	/** browserExportBlockedReason(store): non-null routes to Rust with the reason. */
	blockedReason: string | null;
	/** WebCodecs H.264 encode is usable in this WebView (probeBrowserExportCapability). */
	capabilitySupported: boolean;
}

export interface ExportEngineDecision {
	engine: ExportEngine;
	/** Stable, low-cardinality reason for telemetry (drives the rollout metrics). */
	reason: string;
}

/** Resolve the export engine. First matching guard wins, so the order encodes
 *  precedence: disabled → user-forced → feature-blocked → capability, else browser. */
export function chooseExportEngine(i: ExportEngineInputs): ExportEngineDecision {
	if (!i.masterEnabled) return { engine: "rust", reason: "browser-export-disabled" };
	if (i.forceLegacy) return { engine: "rust", reason: "user-forced-legacy" };
	if (i.blockedReason) return { engine: "rust", reason: `blocked:${i.blockedReason}` };
	if (!i.capabilitySupported) return { engine: "rust", reason: "webcodecs-unsupported" };
	return { engine: "browser", reason: "browser" };
}
