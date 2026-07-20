/**
 * First-party line glyphs for the tool cards, authored as Lucide-style
 * `IconNode` data (24×24 viewBox, currentColor stroke). Drawn in-house so each
 * tool reads as its own operation rather than a generic file icon; rendered via
 * `<LocalIcon iconNode={…} />`. Several slugs share an op (transcode), so they
 * intentionally share a glyph. Keyed by slug — add a tool, add an entry.
 */
import type { IconNode } from '@recast/ui/local-icon';

// Animated frames + play → GIF.
const gif: IconNode = [
	['rect', { x: 7, y: 3, width: 14, height: 11, rx: 2 }],
	['rect', { x: 3, y: 9, width: 14, height: 12, rx: 2 }],
	['path', { d: 'M8 13v4l3.5-2z', fill: 'currentColor', stroke: 'none' }],
];

// Scissors → trim.
const trim: IconNode = [
	['circle', { cx: 6, cy: 6, r: 3 }],
	['circle', { cx: 6, cy: 18, r: 3 }],
	['line', { x1: 20, y1: 4, x2: 8.12, y2: 15.88 }],
	['line', { x1: 14.47, y1: 14.48, x2: 20, y2: 20 }],
	['line', { x1: 8.12, y1: 8.12, x2: 12, y2: 12 }],
];

// Speaker + cross → mute (remove audio).
const mute: IconNode = [
	['path', { d: 'M11 5 6 9H2v6h4l5 4z', fill: 'currentColor', stroke: 'none' }],
	['line', { x1: 22, y1: 9, x2: 16, y2: 15 }],
	['line', { x1: 16, y1: 9, x2: 22, y2: 15 }],
];

// Music note → MP3.
const mp3: IconNode = [
	['path', { d: 'M9 18V5l12-2v13' }],
	['circle', { cx: 6, cy: 18, r: 3 }],
	['circle', { cx: 18, cy: 16, r: 3 }],
];

// Waveform → extract audio.
const waveform: IconNode = [
	['line', { x1: 4, y1: 10, x2: 4, y2: 14 }],
	['line', { x1: 8, y1: 6, x2: 8, y2: 18 }],
	['line', { x1: 12, y1: 3, x2: 12, y2: 21 }],
	['line', { x1: 16, y1: 7, x2: 16, y2: 17 }],
	['line', { x1: 20, y1: 9, x2: 20, y2: 15 }],
];

// Frame with picture → video to images.
const frames: IconNode = [
	['rect', { x: 3, y: 5, width: 18, height: 14, rx: 2 }],
	['circle', { cx: 8.5, cy: 10, r: 1.5 }],
	['path', { d: 'M21 16l-4.5-4.5L8 20' }],
];

// Bidirectional swap → transcode (format conversion).
const convert: IconNode = [
	['path', { d: 'M8 4 4 8l4 4' }],
	['path', { d: 'M4 8h16' }],
	['path', { d: 'M16 20l4-4-4-4' }],
	['path', { d: 'M20 16H4' }],
];

// Diagonal arrows inward → compress.
const compress: IconNode = [
	['polyline', { points: '4 14 10 14 10 20' }],
	['polyline', { points: '20 10 14 10 14 4' }],
	['line', { x1: 14, y1: 10, x2: 21, y2: 3 }],
	['line', { x1: 3, y1: 21, x2: 10, y2: 14 }],
];

// Frame + diagonal handles → resize.
const resize: IconNode = [
	['rect', { x: 3, y: 3, width: 18, height: 18, rx: 2 }],
	['path', { d: 'M8 16 16 8' }],
	['path', { d: 'M11 8h5v5' }],
	['path', { d: 'M13 16H8v-5' }],
];

export const TOOL_ICONS: Record<string, IconNode> = {
	'mp4-to-gif': gif,
	'trim-video': trim,
	'mute-video': mute,
	'mp4-to-mp3': mp3,
	'extract-audio': waveform,
	'video-to-images': frames,
	'mov-to-mp4': convert,
	'mp4-to-webm': convert,
	'webm-to-mp4': convert,
	'compress-video': compress,
	'resize-video': resize,
};

export const toolIcon = (slug: string): IconNode => TOOL_ICONS[slug] ?? convert;
