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
    Zap
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
		{ icon: BarChart3, title: "Watch analytics", description: "Who watched, how far they got, what they replayed." },
		{ icon: Lock, title: "Access controls", description: "Per-viewer access, password gates, link expiry." },
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

	// "OS recorder stops at a file" — contrast rows
	export const contrast = [
		{ os: "A raw .mp4 dumped on your desktop", recast: "A polished demo, framed and padded" },
		{ os: "A jittery, distracting cursor", recast: "Cursor smoothed and snapped to targets" },
		{ os: "Re-pick region, window, mic, camera every take", recast: "Saved recording profiles, one shortcut to switch" },
		{ os: "You, manually trimming in iMovie", recast: "Trim, zoom, and backgrounds, all built in" },
		{ os: "Manual export, upload, and link-fetching", recast: "One click to your Drive, share-link in hand" },
	];

	export const polishFeatures = [
		{ icon: MousePointer2, title: "Cursor refinement", description: "Velocity smoothing kills twitchy paths, snaps to targets." },
		{ icon: Layout, title: "Auto layouts", description: "Padding, backgrounds, framing applied live as you record." },
		{ icon: Zap, title: "Smart zoom", description: "Recast zooms toward the action so viewers never miss the point." },
		{ icon: Scissors, title: "Trim & ship", description: "Cut dead frames, export hardware-encoded MP4 in seconds." },
	];

	// Recording-side superpowers. Two beats — kept short so the section
	// reads as marketing copy, not a feature catalog.
	export const recordingFeatures = [
		{
			icon: Layers,
			title: "Recording profiles",
			description: "Save capture presets and switch with one shortcut. Investor demo, changelog clip, tutorial: pick the profile, hit record.",
		},
		{
			icon: Pause,
			title: "Pause & resume mid-take",
			description: "A knock at the door no longer means re-recording. Paused spans trim out cleanly.",
		},
	];

	export const shareFeatures = [
		{ icon: HardDriveUpload, title: "Upload to your Drive", description: "Connect once. The export dialog ships the file straight to your account, no manual upload." },
		{ icon: Link2, title: "Copy a share link", description: "When the upload finishes, the link is one click away. Send it however you already send links." },
	];

	// "Make it yours" beat — extensions as proof of the no-lock-in moat, not a
	// generic "marketplace". Stays a supporting note under the core wedge.
	export const extensionBeat = [
		{ icon: MousePointer2, title: "Cursor packs", description: "Swap the pointer for a new style. Install a pack and it shows up in the cursor picker." },
		{ icon: Palette, title: "Backgrounds & gradients", description: "Wallpapers, gradients and color sets that drop straight into the background picker." },
		{ icon: Gauge, title: "Motion presets", description: "Easing and cursor-smoothing presets, shared as packs you can install in a click." },
		{ icon: ShieldCheck, title: "Safe by design", description: "Every pack is a manifest plus static files. No code runs, every asset is hash-checked, and nothing asks for permission." },
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
		image: string | null;
	}> = [
		{
			kind: "auto",
			icon: Target,
			title: "Smart zoom on clicks",
			description:
				"Recast watches your cursor, reads clicks and dwell, and zooms toward the moment that matters. You set zero keyframes.",
			image: null,
		},
		{
			kind: "auto",
			icon: VolumeX,
			title: "Silence trimming",
			description:
				"Detects dead-air segments (quiet audio + still cursor) and offers them up as one-click cuts. Toggle them off any time.",
			image: "/screenshots/feat-silence-trim.png",
		},
		{
			kind: "auto",
			icon: MousePointer2,
			title: "Cursor smoothing",
			description:
				"Velocity-aware easing kills the jitter, with optional snap-to-target so the path lands where you meant to point.",
			image: "/screenshots/feat-cursor-smoothing.png",
		},
		{
			kind: "manual",
			icon: Zap,
			title: "Zoom regions on the timeline",
			description:
				"Drag any moment to add a focus region. The auto picks are just a starting point. Every position, scale, and easing is yours to tweak.",
			image: "/screenshots/feat-zoom-regions.png",
		},
		{
			kind: "manual",
			icon: Highlighter,
			title: "Annotations & blur",
			description:
				"Drop arrows, rectangles, text, or a privacy blur straight on the frame. Layers live on the timeline alongside everything else.",
			image: "/screenshots/feat-annotations.png",
		},
		{
			kind: "manual",
			icon: Camera,
			title: "Camera bubble",
			description:
				"Record yourself in a draggable bubble with shape, border, and follow-the-cursor motion. No second app. No green screen.",
			image: "/screenshots/feat-camera-bubble.png",
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
			a: "The app is free forever, no account needed. Recast Cloud (hosted sharing with analytics and access controls) is the paid add-on, coming soon. Today you can share straight to your own Google Drive.",
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