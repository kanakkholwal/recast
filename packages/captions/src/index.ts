/**
 * @recast/captions: the caption model shared by the desktop editor preview, the
 * export burn-in (Rust mirrors these shapes), and the web player overlay. Pure
 * data + arithmetic; no store, no DOM.
 */

export type {
	TranscriptWord,
	CaptionChunk,
	CaptionEmphasis,
	CaptionEntrance,
	CaptionHighlight,
	CaptionAnimation,
	CaptionStyle,
	CaptionPreset,
} from "./types";

export {
	DEFAULT_CAPTION_ANIMATION,
	resolveCaptionAnimation,
	isStaticAnimation,
	chunkWords,
	activeChunkIndex,
	activeWordIndex,
	type CaptionChunkRun,
} from "./chunking";

export { spokenWordCount, karaokeCentiseconds } from "./highlight";
export { breakIntoLines } from "./linebreak";
export { pillBox, type PillBox } from "./geometry";
export { withAlpha } from "./color";
export { wordColor, wordScaled, type WordRenderInput } from "./word-render";
// CaptionBox is deliberately not re-exported: this entry stays pure TS for plain-Node runners. Import it from "@recast/captions/box".
export { parseKaraokeCue, parseVttTime } from "./vtt";
export {
	captionHeightFrac,
	captionTopFrac,
	type VideoRect,
} from "./layout";
export { CAPTION_PRESETS, DEFAULT_CAPTION_STYLE } from "./presets";
