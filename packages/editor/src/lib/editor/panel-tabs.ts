/**
 * The properties-panel section vocabulary.
 *
 * Deliberately its own module rather than part of the editor store: the URL
 * parser and the panel rail both need the list, and importing it from the store
 * drags in that module's whole graph (Tauri IPC, analytics) for a string array.
 *
 * A const array, not a bare union, so `?tab=` can be validated against the real
 * list instead of a hand-copied one that drifts when a tab is added.
 *
 * 'dev' is a dev-build-only tab (experimental OCR review); it is UI state only
 * and is never serialized into a project.
 */
export const PANEL_TABS = [
	"clip",
	"background",
	"focus",
	"annotations",
	"cursor",
	"camera",
	"audio",
	"music",
	"captions",
	"extensions",
	"info",
	"dev",
] as const;

export type PanelTab = (typeof PANEL_TABS)[number];

/**
 * The subset a browser host can serve.
 *
 * Out: `music` and `extensions` (their packs install natively), `info` (no
 * filesystem to reveal), `audio` (its per-track gains describe a recording's
 * system/mic pair, which an uploaded file doesn't have), and `dev`.
 * `cursor` stays: the panel's controls are inert without a cursor track, but it
 * is how the feature is discovered. Captions is import-only — see
 * `EditorServices.transcription`.
 */
export const WEB_PANEL_TABS = [
	"clip",
	"background",
	"focus",
	"annotations",
	"cursor",
	"camera",
	"captions",
] as const satisfies readonly PanelTab[];
