/**
 * The tool catalog. Each entry is one SEO landing page and one conversion.
 * Several slugs can map to the same `op` with different fixed options (e.g.
 * mp4-to-webm and webm-to-mp4 are both `transcode`), which gives us a page per
 * keyword without duplicating logic. The page UI, capability banner, size gate,
 * and option controls are all driven from this data.
 */

import type { ToolRequirements } from "./capabilities";
import type { ToolOp, ToolOptions } from "./worker-protocol";

export interface ToolControl {
	key: keyof ToolOptions;
	label: string;
	type: "number" | "select";
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

const AAC = "mp4a.40.2";
const H264 = "avc1.42001f";
const VP9 = "vp09.00.10.08";

const privacyFaq: ToolFaq = {
	q: "Is my video uploaded to a server?",
	a: "No. The conversion runs entirely in your browser using your device's own video engine. Your file never leaves your computer, and there is nothing for us to store or delete.",
};

/**
 * True of every tool, so they are appended to each entry rather than repeated
 * by hand. Tool-specific answers come first: a visitor who searched for one
 * conversion wants that answer above the boilerplate.
 */
const commonFaqs: ToolFaq[] = [
	{
		q: "Is it free, and is there a watermark?",
		a: "It is free with no watermark, no sign-up, and no export limit. There is nothing to buy here; the paid plan is on the Recast desktop app, which is a different product.",
	},
	{
		q: "How large a file can I convert?",
		a: "The whole file is held in your device memory, so the ceiling depends on your machine. The upload panel shows the limit it worked out for your device, and it will tell you before it starts rather than failing halfway.",
	},
	{
		q: "Which browsers work?",
		a: "Chrome, Edge, and other Chromium browsers have the broadest support. Firefox and Safari handle some operations. The page probes your browser on load and says plainly if a step is unsupported, so you never wait on a conversion that cannot run.",
	},
	{
		q: "Does it work on a phone?",
		a: "It runs, but a phone has far less memory than a laptop, so keep the file small. For anything long, use a desktop browser.",
	},
	{
		q: "Does it work offline?",
		a: "Once the page has loaded, yes. Nothing is fetched during the conversion, so it keeps working with the connection off.",
	},
	{
		q: "Why is my browser fan spinning?",
		a: "Video work is CPU-heavy and it is happening on your machine rather than on a server. That is the trade for not uploading anything. Closing other tabs helps on a long file.",
	},
];

export const TOOLS: ToolDef[] = [
	{
		slug: "mp4-to-gif",
		op: "video-to-gif",
		title: "MP4 to GIF Converter",
		tagline: "Turn a video clip into an animated GIF, right in your browser.",
		description:
			"Convert MP4, MOV, or WebM video to an animated GIF for free. Runs entirely in your browser, no upload, no watermark.",
		accept: "video/*",
		requirements: { tier: "decode", videoDecode: { codec: H264 } },
		outputLabel: "GIF",
		controls: [
			{
				key: "width",
				label: "Width (px)",
				type: "number",
				default: 480,
				min: 64,
				max: 1280,
				step: 16,
			},
			{
				key: "fps",
				label: "Frame rate",
				type: "number",
				default: 12,
				min: 5,
				max: 24,
				step: 1,
				hint: "Lower fps = smaller file.",
			},
		],
		faq: [
			privacyFaq,
			{
				q: "Why is the GIF large?",
				a: "GIF is an old format with no real compression. Keep the width and frame rate modest, or trim the clip first, to keep the size down.",
			},
			{
				q: "What videos can I use?",
				a: "MP4, MOV, and WebM all work. The clip is decoded by your browser, so very long videos are better trimmed first.",
			},
			{
				q: "How long a clip should I convert?",
				a: "Under about 10 seconds. GIF stores every frame as a full image, so length costs more than resolution does. Trim first, then convert.",
			},
			{
				q: "Does the GIF keep the audio?",
				a: "No. The GIF format has no audio track at all. If you need sound, export a WebM or MP4 instead.",
			},
			{
				q: "What frame rate should I pick?",
				a: "12 fps reads as smooth for screen recordings and roughly halves the size against 24. Drop to 8 for a talking-head clip where motion is slight.",
			},
			...commonFaqs,
		],
	},
	{
		slug: "trim-video",
		op: "trim",
		title: "Trim Video Online",
		tagline: "Cut a video to the part you want, with no re-encoding.",
		description:
			"Trim or cut a video online for free. Keeps the original quality, runs in your browser, nothing is uploaded.",
		accept: "video/*",
		requirements: { tier: "container" },
		outputLabel: "video",
		controls: [
			{ key: "startSec", label: "Start (seconds)", type: "number", default: 0, min: 0, step: 0.1 },
			{ key: "endSec", label: "End (seconds)", type: "number", default: 10, min: 0, step: 0.1 },
		],
		faq: [
			privacyFaq,
			{
				q: "Does trimming lose quality?",
				a: "No. Trimming copies the original video and audio without re-encoding, so quality is identical to the source.",
			},
			{
				q: "Why does the cut snap slightly?",
				a: "Fast, lossless trimming cuts on keyframes, so the start can land a fraction of a second early. That avoids re-encoding the whole file.",
			},
			{
				q: "Can I cut a section out of the middle?",
				a: "Not in one pass. This keeps a single range. To remove a middle section, trim the two halves you want and join them in a real editor.",
			},
			{
				q: "Is the audio trimmed too?",
				a: "Yes. Both tracks are cut to the same range and stay in sync, because neither is re-encoded.",
			},
			{
				q: "Why is it so much faster than other trimmers?",
				a: "Nothing is uploaded and nothing is re-encoded. It rewrites the container around the range you picked, which is close to a file copy.",
			},
			...commonFaqs,
		],
	},
	{
		slug: "mute-video",
		op: "mute",
		title: "Mute Video (Remove Audio)",
		tagline: "Strip the sound from a video without touching the picture.",
		description:
			"Remove audio from a video online for free. Keeps the original video quality, runs in your browser, no upload.",
		accept: "video/*",
		requirements: { tier: "container" },
		outputLabel: "video",
		faq: [
			privacyFaq,
			{
				q: "Is the video re-encoded?",
				a: "No. Only the audio track is dropped; the video stream is copied untouched, so there's no quality loss.",
			},
			{
				q: "Will the file get smaller?",
				a: "A little. Audio is usually a small share of a video file, so expect a few percent rather than a dramatic drop. Use the compressor if size is the goal.",
			},
			{
				q: "Can I lower the volume instead of removing it?",
				a: "Not here. This drops the track entirely, which is what most people want before adding a new voiceover or posting to an autoplay feed.",
			},
			{
				q: "Why mute a video at all?",
				a: "Social feeds autoplay muted anyway, background noise is distracting, and a silent track avoids copyright claims on incidental music.",
			},
			...commonFaqs,
		],
	},
	{
		slug: "mp4-to-mp3",
		op: "audio-to-mp3",
		title: "MP4 to MP3 Converter",
		tagline: "Extract the audio from a video as an MP3 file.",
		description:
			"Convert MP4, MOV, or WebM video to MP3 audio for free. Runs in your browser, nothing is uploaded.",
		accept: "video/*,audio/*",
		requirements: { tier: "decode", audioDecode: { codec: AAC } },
		outputLabel: "MP3",
		faq: [
			privacyFaq,
			{
				q: "What bitrate is the MP3?",
				a: "192 kbps, which is transparent for most listening. The audio is decoded from your file and re-encoded to MP3 in your browser.",
			},
			{
				q: "My video has no sound, why did it fail?",
				a: "There has to be an audio track to extract. Screen recordings made without audio won't produce an MP3.",
			},
			{
				q: "Does converting to MP3 lose quality?",
				a: "Slightly, because the audio is decoded and re-encoded. At 192 kbps the difference is inaudible for speech and most music. Use Extract audio and pick M4A if you want a bit-exact copy.",
			},
			{
				q: "Can I convert a whole podcast episode?",
				a: "Long files work but are held in memory while they convert, so a very long recording can exhaust a phone. Use a desktop browser for anything over an hour.",
			},
			{
				q: "Does it keep the title and artist tags?",
				a: "No. The output carries the audio only. Add tags afterwards in your player or music library if you need them.",
			},
			...commonFaqs,
		],
	},
	{
		slug: "extract-audio",
		op: "extract-audio",
		title: "Extract Audio from Video",
		tagline: "Pull the soundtrack out as WAV or M4A.",
		description:
			"Extract audio from a video as WAV or M4A for free. Runs in your browser, no upload, no account.",
		accept: "video/*",
		requirements: { tier: "decode", audioDecode: { codec: AAC } },
		outputLabel: "audio",
		controls: [
			{
				key: "audioFormat",
				label: "Format",
				type: "select",
				default: "m4a",
				options: [
					{ value: "m4a", label: "M4A (smaller, copies the track)" },
					{ value: "wav", label: "WAV (uncompressed)" },
					{ value: "mp3", label: "MP3 (192 kbps)" },
				],
			},
		],
		faq: [
			privacyFaq,
			{
				q: "Which format should I pick?",
				a: "M4A is smallest and copies the original audio with no quality loss. WAV is uncompressed and large. MP3 is the most widely compatible.",
			},
			{
				q: 'What does "copies the track" mean?',
				a: "M4A takes the existing audio out of the video without decoding it, so the result is bit-for-bit the original. WAV and MP3 both re-encode.",
			},
			{
				q: "Why would I want WAV?",
				a: "Editing. WAV is uncompressed, so an editor or transcription tool can work on it without a lossy generation in the middle. Expect roughly 10 MB per minute.",
			},
			{
				q: "Can I extract just one speaker or channel?",
				a: "No. The whole mixed track comes out as it is. Splitting channels or speakers is a job for an audio editor.",
			},
			...commonFaqs,
		],
	},
	{
		slug: "video-to-images",
		op: "extract-frames",
		title: "Video to Images",
		tagline: "Grab evenly spaced frames from a video as a ZIP of images.",
		description:
			"Export frames from a video as PNG or JPG images for free. Runs in your browser, nothing is uploaded.",
		accept: "video/*",
		requirements: { tier: "decode", videoDecode: { codec: H264 } },
		outputLabel: "images (ZIP)",
		controls: [
			{
				key: "frameCount",
				label: "Number of frames",
				type: "number",
				default: 10,
				min: 1,
				max: 50,
				step: 1,
			},
			{
				key: "imageFormat",
				label: "Format",
				type: "select",
				default: "png",
				options: [
					{ value: "png", label: "PNG (lossless)" },
					{ value: "jpeg", label: "JPG (smaller)" },
				],
			},
		],
		faq: [
			privacyFaq,
			{
				q: "How are the frames chosen?",
				a: "They're spaced evenly across the whole video, so you get a representative set from start to finish.",
			},
			{
				q: "Can I grab one exact frame?",
				a: "Set the count to 1 and you get the middle frame. For a specific moment, trim the video to that instant first, then extract.",
			},
			{
				q: "PNG or JPG?",
				a: "PNG is lossless and right for screenshots, UI and text. JPG is a fraction of the size and fine for camera footage.",
			},
			{
				q: "What resolution are the images?",
				a: "The video's own resolution, unscaled. A 4K clip gives 4K stills.",
			},
			{
				q: "Why a ZIP?",
				a: "A browser cannot hand you a folder. One archive keeps the set together and downloads in a single click.",
			},
			...commonFaqs,
		],
	},
	{
		slug: "mov-to-mp4",
		op: "transcode",
		title: "MOV to MP4 Converter",
		tagline: "Convert iPhone and QuickTime MOV files to MP4.",
		description:
			"Convert MOV video to MP4 for free, in your browser. No upload, no watermark, no size cap for the desktop app.",
		accept: "video/quicktime,video/mp4,.mov,.mp4",
		requirements: { tier: "encode", videoEncode: { codec: H264 } },
		fixedOptions: { container: "mp4", videoCodec: "avc", audioCodec: "aac" },
		outputLabel: "MP4",
		faq: [
			privacyFaq,
			{
				q: "Does this work in Safari?",
				a: "Encoding to MP4 needs video-encoder support, which is most reliable in Chrome and Edge today. The page will tell you if your browser can't do it.",
			},
			{
				q: "Why will nothing play my iPhone MOV?",
				a: "iPhones record MOV, often with HEVC inside. Windows apps, older editors and many web players expect MP4 with H.264. This converts to exactly that.",
			},
			{
				q: "Does converting lose quality?",
				a: "A generation, yes: the video is decoded and re-encoded. It is visually close for normal viewing, but do it once from the original rather than repeatedly.",
			},
			{
				q: "Will it still play on my iPhone afterwards?",
				a: "Yes. MP4 with H.264 plays on every iPhone, Android, TV and browser worth naming. That is the point of converting.",
			},
			...commonFaqs,
		],
	},
	{
		slug: "mp4-to-webm",
		op: "transcode",
		title: "MP4 to WebM Converter",
		tagline: "Convert MP4 to WebM (VP9 + Opus) for the web.",
		description: "Convert MP4 video to WebM for free, in your browser. No upload, no account.",
		accept: "video/*",
		requirements: { tier: "encode", videoEncode: { codec: VP9 } },
		fixedOptions: { container: "webm", videoCodec: "vp9", audioCodec: "opus" },
		outputLabel: "WebM",
		faq: [
			privacyFaq,
			{
				q: "Why convert to WebM?",
				a: "WebM with VP9 is an open, royalty-free format that's well supported on the web and often smaller than MP4 at the same quality.",
			},
			{
				q: "Does WebM play everywhere?",
				a: "Every modern browser plays it, including Safari on recent versions. Outside the browser it is patchier: some TVs, older phones and editors will not open it.",
			},
			{
				q: "How much smaller is VP9 than H.264?",
				a: "Commonly 20 to 35 percent at the same visual quality, and more on flat, screen-recorded content. Encoding takes longer in exchange.",
			},
			{
				q: "Is WebM good for a website background video?",
				a: "Yes, and it is the usual choice. Ship WebM first with an MP4 fallback in a second source tag for anything that cannot decode VP9.",
			},
			...commonFaqs,
		],
	},
	{
		slug: "webm-to-mp4",
		op: "transcode",
		title: "WebM to MP4 Converter",
		tagline: "Convert WebM to MP4 (H.264 + AAC) for wide compatibility.",
		description: "Convert WebM video to MP4 for free, in your browser. No upload, no watermark.",
		accept: "video/webm,video/*",
		requirements: { tier: "encode", videoEncode: { codec: H264 } },
		fixedOptions: { container: "mp4", videoCodec: "avc", audioCodec: "aac" },
		outputLabel: "MP4",
		faq: [
			privacyFaq,
			{
				q: "Why MP4?",
				a: "MP4 with H.264 plays just about everywhere: phones, TVs, editors, and social platforms.",
			},
			{
				q: "My screen recording is a WebM and Premiere will not open it. Will this fix that?",
				a: "Yes, that is the common case. Browser and OBS recordings are often WebM, which most editors refuse. MP4 with H.264 imports into every editor.",
			},
			{
				q: "Can I upload WebM to Instagram or TikTok?",
				a: "Not reliably. Both expect MP4. Convert first and the upload stops silently failing.",
			},
			{
				q: "Will the file get bigger?",
				a: "Often slightly, because H.264 is less efficient than VP9. You are trading a little size for playing everywhere.",
			},
			...commonFaqs,
		],
	},
	{
		slug: "compress-video",
		op: "compress",
		title: "Compress Video Online",
		tagline: "Shrink a video's file size by lowering its bitrate.",
		description:
			"Compress a video to a smaller file for free, in your browser. No upload, no watermark.",
		accept: "video/*",
		requirements: { tier: "encode", videoEncode: { codec: H264 } },
		controls: [
			{
				key: "videoBitrate",
				label: "Target bitrate",
				type: "select",
				default: "1500000",
				options: [
					{ value: "800000", label: "Small (0.8 Mbps)" },
					{ value: "1500000", label: "Balanced (1.5 Mbps)" },
					{ value: "3000000", label: "High (3 Mbps)" },
				],
			},
		],
		outputLabel: "video",
		faq: [
			privacyFaq,
			{
				q: "How much smaller will it get?",
				a: "It depends on the source, but lowering the bitrate is the main lever. Start with Balanced and drop to Small if you need a tighter file.",
			},
			{
				q: "Which bitrate should I choose?",
				a: "Balanced at 1.5 Mbps suits 1080p screen recordings and talking heads. Small at 0.8 Mbps is for email and chat limits. High at 3 Mbps keeps detail in fast motion and camera footage.",
			},
			{
				q: "Will it look worse?",
				a: "Some. Compression removes detail, and it shows first in fast motion and fine text. Screen recordings hold up well because most of the frame is static.",
			},
			{
				q: "How do I hit a specific file size?",
				a: "Roughly: bitrate in Mbps times duration in seconds, divided by 8, gives megabytes. A 60-second clip at 1.5 Mbps lands near 11 MB before audio.",
			},
			{
				q: "Should I resize instead of compressing?",
				a: "If the video will be watched small, yes. Halving the dimensions cuts far more than lowering bitrate does, and it looks cleaner than a heavily compressed large frame.",
			},
			...commonFaqs,
		],
	},
	{
		slug: "resize-video",
		op: "resize",
		title: "Resize Video Online",
		tagline: "Scale a video to a new width and height.",
		description: "Resize or scale a video for free, in your browser. No upload, no account.",
		accept: "video/*",
		requirements: { tier: "encode", videoEncode: { codec: H264 } },
		controls: [
			{
				key: "width",
				label: "Width (px)",
				type: "number",
				default: 1280,
				min: 64,
				max: 3840,
				step: 2,
			},
			{
				key: "height",
				label: "Height (px)",
				type: "number",
				default: 720,
				min: 64,
				max: 2160,
				step: 2,
			},
		],
		outputLabel: "video",
		faq: [
			privacyFaq,
			{
				q: "Will it stretch my video?",
				a: "The video is fit inside the dimensions you choose without distortion. Match the aspect ratio to avoid letterboxing.",
			},
			{
				q: "What size should I use for social?",
				a: "1080 by 1920 for Reels, TikTok and Shorts. 1080 by 1080 for a square feed post. 1920 by 1080 for YouTube and anything landscape.",
			},
			{
				q: "Can I make a video larger?",
				a: "You can set bigger numbers, but nothing is gained. Upscaling stretches the pixels that are already there and usually looks softer than the original.",
			},
			{
				q: "Why must the width and height be even?",
				a: "H.264 encodes in 2-pixel blocks, so odd dimensions are not representable. The step is set to 2 to keep every value valid.",
			},
			{
				q: "Does resizing shrink the file?",
				a: "Substantially. Halving both dimensions is a quarter of the pixels, and the file usually falls by a similar order. It is the most effective size lever there is.",
			},
			...commonFaqs,
		],
	},
];

export const toolBySlug = (slug: string): ToolDef | undefined => TOOLS.find((t) => t.slug === slug);
