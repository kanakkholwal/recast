/**
 * Export-engine resolver (migration Phase 1): the single decision for whether an
 * export runs through the browser compositor (RenderCore, WYSIWYG with the
 * preview) or the legacy Rust/FFmpeg compositor. Pure and dependency-free so it
 * unit-tests without the store/DOM; the caller feeds the master flag, the
 * escape-hatch setting, the feature-gate reason, and the WebCodecs capability.
 *
 * Rust stays only as an automatic fallback (capability / feature-gate / user
 * escape hatch) — never a user-facing engine choice.
 */

export type ExportEngine = "browser" | "rust";

export interface ExportEngineInputs {
	/** The `browserExportBeta` experimental flag. While false, everything is Rust. */
	masterEnabled: boolean;
	/** Support escape hatch for when browser export flips default-on. No producer
	 *  yet, so omitting it is the normal case. */
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
