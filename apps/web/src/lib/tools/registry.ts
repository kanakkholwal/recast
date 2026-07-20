/**
 * The tool catalog. Each entry is one SEO landing page and one conversion.
 * Several slugs can map to the same `op` with different fixed options (e.g.
 * mp4-to-webm and webm-to-mp4 are both `transcode`), which gives us a page per
 * keyword without duplicating logic. The page UI, capability banner, size gate,
 * and option controls are all driven from this data.
 */

import type { ToolRequirements } from './capabilities';
import type { ToolOp, ToolOptions } from './worker-protocol';

export interface ToolControl {
	key: keyof ToolOptions;
	label: string;
	type: 'number' | 'select';
	default: number | string;
	min?: number;
	max?: number;
	step?: number;
	options?: { value: string; label: string }[];
	hint?: string;
}

export interface ToolFaq {
	q: string;
	a: string;
}

export interface ToolDef {
	slug: string;
	op: ToolOp;
	/** Used as <title> and the page H1. */
	title: string;
	tagline: string;
	/** Meta description. */
	description: string;
	/** File input accept attribute. */
	accept: string;
	requirements: ToolRequirements;
	/** Options baked in for this slug (not user-editable). */
	fixedOptions?: ToolOptions;
	/** User-adjustable options rendered as controls. */
	controls?: ToolControl[];
	/** Short label for the output, e.g. "GIF". */
	outputLabel: string;
	faq: ToolFaq[];
}

const AAC = 'mp4a.40.2';
const H264 = 'avc1.42001f';
const VP9 = 'vp09.00.10.08';

const privacyFaq: ToolFaq = {
	q: 'Is my video uploaded to a server?',
	a: "No. The conversion runs entirely in your browser using your device's own video engine. Your file never leaves your computer.",
};

export const TOOLS: ToolDef[] = [
	{
		slug: 'mp4-to-gif',
		op: 'video-to-gif',
		title: 'MP4 to GIF Converter',
		tagline: 'Turn a video clip into an animated GIF, right in your browser.',
		description:
			'Convert MP4, MOV, or WebM video to an animated GIF for free. Runs entirely in your browser, no upload, no watermark.',
		accept: 'video/*',
		requirements: { tier: 'decode', videoDecode: { codec: H264 } },
		outputLabel: 'GIF',
		controls: [
			{
				key: 'width',
				label: 'Width (px)',
				type: 'number',
				default: 480,
				min: 64,
				max: 1280,
				step: 16,
			},
			{
				key: 'fps',
				label: 'Frame rate',
				type: 'number',
				default: 12,
				min: 5,
				max: 24,
				step: 1,
				hint: 'Lower fps = smaller file.',
			},
		],
		faq: [
			privacyFaq,
			{
				q: 'Why is the GIF large?',
				a: 'GIF is an old format with no real compression. Keep the width and frame rate modest, or trim the clip first, to keep the size down.',
			},
			{
				q: 'What videos can I use?',
				a: 'MP4, MOV, and WebM all work. The clip is decoded by your browser, so very long videos are better trimmed first.',
			},
		],
	},
	{
		slug: 'trim-video',
		op: 'trim',
		title: 'Trim Video Online',
		tagline: 'Cut a video to the part you want, with no re-encoding.',
		description:
			'Trim or cut a video online for free. Keeps the original quality, runs in your browser, nothing is uploaded.',
		accept: 'video/*',
		requirements: { tier: 'container' },
		outputLabel: 'video',
		controls: [
			{ key: 'startSec', label: 'Start (seconds)', type: 'number', default: 0, min: 0, step: 0.1 },
			{ key: 'endSec', label: 'End (seconds)', type: 'number', default: 10, min: 0, step: 0.1 },
		],
		faq: [
			privacyFaq,
			{
				q: 'Does trimming lose quality?',
				a: 'No. Trimming copies the original video and audio without re-encoding, so quality is identical to the source.',
			},
			{
				q: 'Why does the cut snap slightly?',
				a: 'Fast, lossless trimming cuts on keyframes, so the start can land a fraction of a second early. That avoids re-encoding the whole file.',
			},
		],
	},
	{
		slug: 'mute-video',
		op: 'mute',
		title: 'Mute Video (Remove Audio)',
		tagline: 'Strip the sound from a video without touching the picture.',
		description:
			'Remove audio from a video online for free. Keeps the original video quality, runs in your browser, no upload.',
		accept: 'video/*',
		requirements: { tier: 'container' },
		outputLabel: 'video',
		faq: [
			privacyFaq,
			{
				q: 'Is the video re-encoded?',
				a: "No. Only the audio track is dropped; the video stream is copied untouched, so there's no quality loss.",
			},
		],
	},
	{
		slug: 'mp4-to-mp3',
		op: 'audio-to-mp3',
		title: 'MP4 to MP3 Converter',
		tagline: 'Extract the audio from a video as an MP3 file.',
		description:
			'Convert MP4, MOV, or WebM video to MP3 audio for free. Runs in your browser, nothing is uploaded.',
		accept: 'video/*,audio/*',
		requirements: { tier: 'decode', audioDecode: { codec: AAC } },
		outputLabel: 'MP3',
		faq: [
			privacyFaq,
			{
				q: 'What bitrate is the MP3?',
				a: '192 kbps, which is transparent for most listening. The audio is decoded from your file and re-encoded to MP3 in your browser.',
			},
			{
				q: 'My video has no sound, why did it fail?',
				a: "There has to be an audio track to extract. Screen recordings made without audio won't produce an MP3.",
			},
		],
	},
	{
		slug: 'extract-audio',
		op: 'extract-audio',
		title: 'Extract Audio from Video',
		tagline: 'Pull the soundtrack out as WAV or M4A.',
		description:
			'Extract audio from a video as WAV or M4A for free. Runs in your browser, no upload, no account.',
		accept: 'video/*',
		requirements: { tier: 'decode', audioDecode: { codec: AAC } },
		outputLabel: 'audio',
		controls: [
			{
				key: 'audioFormat',
				label: 'Format',
				type: 'select',
				default: 'm4a',
				options: [
					{ value: 'm4a', label: 'M4A (smaller, copies the track)' },
					{ value: 'wav', label: 'WAV (uncompressed)' },
					{ value: 'mp3', label: 'MP3 (192 kbps)' },
				],
			},
		],
		faq: [
			privacyFaq,
			{
				q: 'Which format should I pick?',
				a: 'M4A is smallest and copies the original audio with no quality loss. WAV is uncompressed and large. MP3 is the most widely compatible.',
			},
		],
	},
	{
		slug: 'video-to-images',
		op: 'extract-frames',
		title: 'Video to Images',
		tagline: 'Grab evenly spaced frames from a video as a ZIP of images.',
		description:
			'Export frames from a video as PNG or JPG images for free. Runs in your browser, nothing is uploaded.',
		accept: 'video/*',
		requirements: { tier: 'decode', videoDecode: { codec: H264 } },
		outputLabel: 'images (ZIP)',
		controls: [
			{
				key: 'frameCount',
				label: 'Number of frames',
				type: 'number',
				default: 10,
				min: 1,
				max: 50,
				step: 1,
			},
			{
				key: 'imageFormat',
				label: 'Format',
				type: 'select',
				default: 'png',
				options: [
					{ value: 'png', label: 'PNG (lossless)' },
					{ value: 'jpeg', label: 'JPG (smaller)' },
				],
			},
		],
		faq: [
			privacyFaq,
			{
				q: 'How are the frames chosen?',
				a: "They're spaced evenly across the whole video, so you get a representative set from start to finish.",
			},
		],
	},
	{
		slug: 'mov-to-mp4',
		op: 'transcode',
		title: 'MOV to MP4 Converter',
		tagline: 'Convert iPhone and QuickTime MOV files to MP4.',
		description:
			'Convert MOV video to MP4 for free, in your browser. No upload, no watermark, no size cap for the desktop app.',
		accept: 'video/quicktime,video/mp4,.mov,.mp4',
		requirements: { tier: 'encode', videoEncode: { codec: H264 } },
		fixedOptions: { container: 'mp4', videoCodec: 'avc', audioCodec: 'aac' },
		outputLabel: 'MP4',
		faq: [
			privacyFaq,
			{
				q: 'Does this work in Safari?',
				a: "Encoding to MP4 needs video-encoder support, which is most reliable in Chrome and Edge today. The page will tell you if your browser can't do it.",
			},
		],
	},
	{
		slug: 'mp4-to-webm',
		op: 'transcode',
		title: 'MP4 to WebM Converter',
		tagline: 'Convert MP4 to WebM (VP9 + Opus) for the web.',
		description: 'Convert MP4 video to WebM for free, in your browser. No upload, no account.',
		accept: 'video/*',
		requirements: { tier: 'encode', videoEncode: { codec: VP9 } },
		fixedOptions: { container: 'webm', videoCodec: 'vp9', audioCodec: 'opus' },
		outputLabel: 'WebM',
		faq: [
			privacyFaq,
			{
				q: 'Why convert to WebM?',
				a: "WebM with VP9 is an open, royalty-free format that's well supported on the web and often smaller than MP4 at the same quality.",
			},
		],
	},
	{
		slug: 'webm-to-mp4',
		op: 'transcode',
		title: 'WebM to MP4 Converter',
		tagline: 'Convert WebM to MP4 (H.264 + AAC) for wide compatibility.',
		description: 'Convert WebM video to MP4 for free, in your browser. No upload, no watermark.',
		accept: 'video/webm,video/*',
		requirements: { tier: 'encode', videoEncode: { codec: H264 } },
		fixedOptions: { container: 'mp4', videoCodec: 'avc', audioCodec: 'aac' },
		outputLabel: 'MP4',
		faq: [
			privacyFaq,
			{
				q: 'Why MP4?',
				a: 'MP4 with H.264 plays just about everywhere: phones, TVs, editors, and social platforms.',
			},
		],
	},
	{
		slug: 'compress-video',
		op: 'compress',
		title: 'Compress Video Online',
		tagline: "Shrink a video's file size by lowering its bitrate.",
		description:
			'Compress a video to a smaller file for free, in your browser. No upload, no watermark.',
		accept: 'video/*',
		requirements: { tier: 'encode', videoEncode: { codec: H264 } },
		controls: [
			{
				key: 'videoBitrate',
				label: 'Target bitrate',
				type: 'select',
				default: '1500000',
				options: [
					{ value: '800000', label: 'Small (0.8 Mbps)' },
					{ value: '1500000', label: 'Balanced (1.5 Mbps)' },
					{ value: '3000000', label: 'High (3 Mbps)' },
				],
			},
		],
		outputLabel: 'video',
		faq: [
			privacyFaq,
			{
				q: 'How much smaller will it get?',
				a: 'It depends on the source, but lowering the bitrate is the main lever. Start with Balanced and drop to Small if you need a tighter file.',
			},
		],
	},
	{
		slug: 'resize-video',
		op: 'resize',
		title: 'Resize Video Online',
		tagline: 'Scale a video to a new width and height.',
		description: 'Resize or scale a video for free, in your browser. No upload, no account.',
		accept: 'video/*',
		requirements: { tier: 'encode', videoEncode: { codec: H264 } },
		controls: [
			{
				key: 'width',
				label: 'Width (px)',
				type: 'number',
				default: 1280,
				min: 64,
				max: 3840,
				step: 2,
			},
			{
				key: 'height',
				label: 'Height (px)',
				type: 'number',
				default: 720,
				min: 64,
				max: 2160,
				step: 2,
			},
		],
		outputLabel: 'video',
		faq: [
			privacyFaq,
			{
				q: 'Will it stretch my video?',
				a: 'The video is fit inside the dimensions you choose without distortion. Match the aspect ratio to avoid letterboxing.',
			},
		],
	},
];

export const toolBySlug = (slug: string): ToolDef | undefined => TOOLS.find((t) => t.slug === slug);
