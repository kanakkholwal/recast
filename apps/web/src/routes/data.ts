import {
	Apple,
	BarChart3,
	Camera,
	Code2,
	Cpu,
	EyeOff,
	Gauge,
	HardDrive,
	HardDriveUpload,
	Highlighter,
	Layers,
	Layout,
	Link2,
	Lock,
	Monitor,
	MousePointer2,
	Palette,
	Pause,
	Rocket,
	Scissors,
	ShieldCheck,
	Target,
	Terminal,
	UserX,
	VolumeX,
	Wand2,
	Zap,
} from "@recast/icons";
import { GithubBrand } from "@recast/ui/brand-icons";

export type BeforeAfterTone = "raw" | "polished";
export const beforeAfterClips: Array<{
	src: string;
	label: string;
	hint: string;
	tone: BeforeAfterTone;
	durationLabel: string;
	applied: string[];
}> = [
	{
		src: "https://acfj680407.ufs.sh/f/04eGlAvZnRytceM29W6qY5xCbfENa2zoprGTi40P83dsVmke",
		label: "Raw recording",
		hint: "Built-in recorder, no edits",
		tone: "raw",
		durationLabel: "0:30",
		applied: [],
	},
	{
		src: "https://acfj680407.ufs.sh/f/04eGlAvZnRytzbxdocCKDxX4WQEiGNYtkVuvdIMnT5sclJge",
		label: "Polished automatically",
		hint: "Same take, run through Recast",
		tone: "polished",
		durationLabel: "~0:22",
		applied: ["Smart zoom", "Cursor smoothing", "Silence trim"],
	},
];

// Cloud preview features — the "more than a Drive link" promise. Kept
// short on purpose: this is a teaser, not a feature page.
export const cloudFeatures = [
	{
		icon: BarChart3,
		title: "Watch analytics",
		description: "Who watched, how far they got, what they replayed.",
	},
	{
		icon: Lock,
		title: "Access controls",
		description: "Per-viewer access, password gates, link expiry.",
	},
	{
		icon: HardDrive,
		title: "Bring your own storage",
		description: "Google Drive today, S3 and R2 planned. Or let Recast host it.",
	},
];

// Storage tiers — Cloud is intentionally storage-agnostic. Free users
// bring their own (the Drive flow already shipping today, plus Cloudinary
// and autorender.io as additional BYO destinations on the roadmap).
// Paid users get Recast-hosted storage and the option to point uploads
// at their own S3 / R2 / Azure / GCP bucket.
export const storageTiers = [
	{
		tier: "Free with Cloud",
		tone: "muted",
		label: "Bring your own storage",
		lines: [
			"Google Drive (shipping today)",
			"Cloudinary, autorender.io (planned)",
			"Your account, your storage, your retention",
		],
	},
	{
		tier: "Paid plans",
		tone: "primary",
		label: "Recast-hosted or your own bucket",
		lines: [
			"Recast-managed storage",
			"Custom S3, R2, Azure Blob, GCP",
			"Data residency in one workspace bill",
		],
	},
];

export const founderUse = [
	{
		icon: Rocket,
		title: "For solo founders",
		description:
			"Investor walkthroughs and demos that look funded. Record one, ship it the same morning.",
	},
	{
		icon: Code2,
		title: "For indie hackers",
		description:
			"Launch videos, changelog clips, Twitter cuts. Ship at midnight, fix typos at 2 AM.",
	},
	{
		icon: Terminal,
		title: "For dev teams & DevRel",
		description:
			"Changelog clips, release notes, tutorials users actually watch. Record the feature, ship the walkthrough.",
	},
];

// Open-source values strip. Sits between the proof shot and the
// tech-stack logo row. Different signal: the logos say "what we're
// built on", this strip says "what that buys you as a user".
export const openSourceClaims = [
	{ icon: GithubBrand, label: "GPLv3 open source" },
	{ icon: Cpu, label: "Tauri + Rust" },
	{ icon: EyeOff, label: "No telemetry" },
	{ icon: HardDrive, label: "Files never leave your machine" },
	{ icon: UserX, label: "No account required" },
];

// Platform-split download buttons for the final CTA. Mirrors the
// stability semantics in /download so the marketing voice never
// over-promises the macOS or Linux builds.
export const platformDownloads = [
	{
		os: "Windows",
		icon: Monitor,
		href: "/download?os=windows",
		variant: "default" as const,
		stability: "stable" as const,
	},
	{
		os: "macOS",
		icon: Apple,
		href: "/download?os=macos",
		variant: "dark" as const,
		stability: "beta" as const,
	},
	{
		os: "Linux",
		icon: Terminal,
		href: "/download?os=linux",
		variant: "dark" as const,
		stability: "beta" as const,
	},
];

export const stabilityChip: Record<"stable" | "beta", { label: string; cls: string }> = {
	stable: {
		label: "Stable",
		cls: "bg-emerald-500/12 text-emerald-600 ring-emerald-500/25 dark:text-emerald-400",
	},
	beta: {
		label: "Beta",
		cls: "bg-amber-500/12 text-amber-600 ring-amber-500/25 dark:text-amber-400",
	},
};

// `cls` above assumes a page/card surface. Inside a filled button the chip
// sits on primary or foreground, where emerald-600 and amber-600 both fail
// contrast — so there it derives from the button's own ink instead, and the
// label is what distinguishes stable from beta.
export const stabilityChipOnFill = "bg-current/20 ring-current/35";

// "OS recorder stops at a file" — contrast rows
export const contrast = [
	{ os: "A raw .mp4 dumped on your desktop", recast: "A polished demo, framed and padded" },
	{ os: "A jittery, distracting cursor", recast: "Cursor smoothed and snapped to targets" },
	{
		os: "Re-pick region, window, mic, camera every take",
		recast: "Saved recording profiles, one shortcut to switch",
	},
	{ os: "You, manually trimming in iMovie", recast: "Trim, zoom, and backgrounds, all built in" },
	{
		os: "Manual export, upload, and link-fetching",
		recast: "One click to your Drive, share-link in hand",
	},
];

export const polishFeatures = [
	{
		icon: MousePointer2,
		title: "Cursor refinement",
		description: "Velocity smoothing kills twitchy paths, snaps to targets.",
	},
	{
		icon: Layout,
		title: "Auto layouts",
		description: "Padding, backgrounds, framing applied live as you record.",
	},
	{
		icon: Zap,
		title: "Smart zoom",
		description: "Recast zooms toward the action so viewers never miss the point.",
	},
	{
		icon: Scissors,
		title: "Trim & ship",
		description: "Cut dead frames, export hardware-encoded MP4 in seconds.",
	},
];

// Recording-side superpowers. Two beats — kept short so the section
// reads as marketing copy, not a feature catalog.
export const recordingFeatures = [
	{
		icon: Layers,
		title: "Recording profiles",
		description: "Save capture presets and switch with one shortcut. Pick a profile, hit record.",
	},
	{
		icon: Pause,
		title: "Pause & resume mid-take",
		description: "A knock at the door no longer means re-recording. Paused spans trim out cleanly.",
	},
];

export const shareFeatures = [
	{
		icon: HardDriveUpload,
		title: "Upload to your Drive",
		description:
			"Connect once. The export dialog ships the file straight to your account, no manual upload.",
	},
	{
		icon: Link2,
		title: "Copy a share link",
		description:
			"When the upload finishes, the link is one click away. Send it however you already send links.",
	},
];

// "Make it yours" beat — extensions as proof of the no-lock-in moat, not a
// generic "marketplace". Stays a supporting note under the core wedge.
export const extensionBeat = [
	{
		icon: MousePointer2,
		title: "Cursor packs",
		description:
			"Swap the pointer for a new style. Install a pack and it shows up in the cursor picker.",
	},
	{
		icon: Palette,
		title: "Backgrounds & gradients",
		description:
			"Wallpapers, gradients and color sets that drop straight into the background picker.",
	},
	{
		icon: Gauge,
		title: "Motion presets",
		description: "Easing and cursor-smoothing presets, shared as packs you can install in a click.",
	},
	{
		icon: ShieldCheck,
		title: "Safe by design",
		description: "A manifest plus static files. No code runs and every asset is hash-checked.",
	},
];

// "Inside the editor" — honest tour of every tool a non-editor user will
// actually touch. Each card is tagged `auto` (it happens for you) or
// `manual` (you reach for it when you want control).
//
// SCREENSHOT ASSET SLOTS — drop these PNG files into static/screenshots/
// to light up each card. Until a file exists, the matching card falls
// back to its `icon` rendered as the hero glyph (still looks deliberate,
// not "missing image"). Target dimensions: 880×560 (16:10), tightly
// cropped to the feature, dark mode. PNG ≤300 KB.
//
//   feat-smart-zoom.png       — editor canvas mid-zoom toward a click
//   feat-silence-trim.png     — timeline with silence regions highlighted
//   feat-cursor-smoothing.png — split / before-after of cursor path
//   feat-zoom-regions.png     — timeline with a manual focus region
//   feat-annotations.png      — frame with arrow + circle + text overlay
//   feat-camera-bubble.png    — webcam bubble showing shape/border options
export type FeatureKind = "auto" | "manual";
export const editorFeatures: Array<{
	kind: FeatureKind;
	icon: typeof Target;
	title: string;
	description: string;
}> = [
	{
		kind: "auto",
		icon: Target,
		title: "Smart zoom on clicks",
		description:
			"Reads clicks and dwell, then zooms toward the moment that matters. Zero keyframes.",
	},
	{
		kind: "auto",
		icon: VolumeX,
		title: "Silence trimming",
		description: "Finds dead air (quiet audio, still cursor) and offers one-click cuts.",
	},
	{
		kind: "auto",
		icon: MousePointer2,
		title: "Cursor smoothing",
		description:
			"Velocity-aware easing kills the jitter and lands the path where you meant to point.",
	},
	{
		kind: "manual",
		icon: Zap,
		title: "Zoom regions on the timeline",
		description: "Drag any moment to add a focus region. Position, scale and easing are all yours.",
	},
	{
		kind: "manual",
		icon: Highlighter,
		title: "Annotations & blur",
		description:
			"Drop arrows, boxes, text or a privacy blur on the frame. Layers live on the timeline.",
	},
	{
		kind: "manual",
		icon: Camera,
		title: "Camera bubble",
		description:
			"A draggable camera bubble with shape, border and cursor-following motion. No second app.",
	},
];

// FAQ. Answers map to claims made elsewhere on this page (offline, free app +
// paid Cloud, auto-polish, camera, platforms) so nothing here over-promises.
// One-open-at-a-time; first item open so the pattern reads on load.
export const faqs: Array<{ q: string; a: string }> = [
	{
		q: "Is Recast a browser extension or a desktop app?",
		a: "A desktop app for macOS, Windows, and Linux. Native capture, fully offline, no extension to install.",
	},
	{
		q: "What can I make with Recast?",
		a: "Product demos, launch and changelog clips, onboarding and tutorial videos, support replies, investor walkthroughs.",
	},
	{
		q: "Do I need an account or an internet connection?",
		a: "Neither. Record, edit, and export entirely offline. Your files stay on your machine until you share them.",
	},
	{
		q: "How is Recast different from Loom or Screen Studio?",
		a: "It polishes while you record. Smart zoom, cursor smoothing, silence trimming apply as you go. Free and offline, not a hosted subscription.",
	},
	{
		q: "Is it really free? What costs money?",
		a: "The app is free forever, no account needed. Recast Cloud, the hosted sharing add-on, is coming soon. Today you share straight to your own Google Drive.",
	},
	{
		q: "Can I record my camera and mic too?",
		a: "Yes. Camera, microphone, and system audio on one timeline, with a draggable webcam bubble you can shape and place.",
	},
	{
		q: "Do I need video editing skills?",
		a: "No. Auto-polish handles most of it. The timeline is small and friendly, not a pro editor.",
	},
	{
		q: "Which platforms are supported?",
		a: "Windows is stable. macOS and Linux are in active beta. Every build is on the download page.",
	},
];

// FAQPage schema. Built from the same `faqs` array the section renders, so
// the structured data always matches the visible copy (Google requires it).
export const faqJsonLd = JSON.stringify({
	"@context": "https://schema.org",
	"@type": "FAQPage",
	mainEntity: faqs.map((f) => ({
		"@type": "Question",
		name: f.q,
		acceptedAnswer: { "@type": "Answer", text: f.a },
	})),
});

export const kindChip: Record<FeatureKind, { label: string; dot: string; ring: string }> = {
	auto: {
		label: "Automatic",
		dot: "bg-emerald-500",
		ring: "text-emerald-600 ring-emerald-500/25 dark:text-emerald-400",
	},
	manual: {
		label: "Manual",
		dot: "bg-foreground/50",
		ring: "text-foreground/70 ring-border-low/60",
	},
};

// Three-up detail rows closing each pillar section. Deliberately separate from
// the longer `recordingFeatures` / `polishFeatures` / `shareFeatures` copy: this
// row is scanned, not read, so each line is one claim.
export const recordColumns = [
	{
		icon: Monitor,
		title: "Region, window, or screen",
		description: "One shortcut starts the capture. No project, no codec picker, no account.",
		href: "/features",
		linkLabel: "See capture",
	},
	{
		icon: Layers,
		title: "Recording profiles",
		description:
			"Save capture presets and switch with one shortcut between demo, clip, and tutorial.",
		href: "/features",
	},
	{
		icon: Pause,
		title: "Pause and resume",
		description: "A knock at the door no longer means re-recording. Paused spans trim out cleanly.",
		href: "/features",
	},
];

export const polishColumns = [
	{
		icon: Zap,
		title: "Smart zoom",
		description: "Recast pushes in toward the action so viewers never miss the point.",
		href: "/features",
	},
	{
		icon: MousePointer2,
		title: "Cursor refinement",
		description: "Velocity smoothing kills twitchy paths and snaps the pointer to its target.",
		href: "/features",
	},
	{
		icon: VolumeX,
		title: "Silence cuts",
		description:
			"Dead air is detected and trimmed, so the finished take lands shorter than the raw one.",
		href: "/features",
	},
];

export const shareColumns = [
	{
		icon: HardDriveUpload,
		title: "Straight to your Drive",
		description: "Connect once. Exports upload to your own account, not to a server we own.",
		href: "/pricing",
	},
	{
		icon: Link2,
		title: "Link in one click",
		description: "The share link is ready the moment the upload finishes. No second tab.",
		href: "/pricing",
	},
	{
		icon: BarChart3,
		title: "Watch analytics",
		description:
			"Recast Cloud adds view counts, per-viewer access, and link expiry when a raw link stops being enough.",
		href: "/pricing",
		linkLabel: "See Cloud",
	},
];
