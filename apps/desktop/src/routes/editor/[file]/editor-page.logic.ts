/**
 * Pure helpers for the editor route. Layout parsing lives in the package
 * (`@recast/editor/editor-shell.logic`) so the route and `<Editor />` can't
 * drift; the export state machine and audio-sync effects stay in the component.
 */

/** Rotating status messages shown below the progress ring during encode. */
export const ENCODE_MESSAGES = [
	"Crunching frames",
	"Encoding pixels",
	"Weaving the timeline",
	"Tuning the colours",
	"Squeezing the bitrate",
	"Polishing every frame",
];

/** Final path segment across both `/` and `\` separators. */
export function basename(path: string): string | undefined {
	return path.split(/[\\/]/).pop();
}
