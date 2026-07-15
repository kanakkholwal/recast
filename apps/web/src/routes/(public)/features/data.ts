// Static data for the /features page. Extracted from +page.svelte so
// the page file stays focused on layout. Each export is its own const so
// the bundler can tree-shake whatever the page doesn't actually use.

import { Camera, Crop, Cpu, FileBox, HardDrive, HardDriveUpload, Highlighter, Layers, Layout, MemoryStick, Monitor, MousePointer2, Pause, Scissors, ShieldCheck, Sparkles, Target, VolumeX, WifiOff, Zap, Apple,  Wand2, Keyboard, UserX } from "@lucide/svelte";
import { GithubBrand } from "@recast/ui/brand-icons";

export const pillars = [
	{
		icon: Wand2,
		title: "Auto-polish on the way in",
		description:
			"Smart zoom, cursor smoothing, silence cuts happen while you record. The demo is mostly done by the time you stop.",
		tags: ["Smart zoom", "Cursor smoothing", "Silence cuts"],
	},
	{
		icon: Layers,
		title: "Recording profiles + pause / resume",
		description:
			"Save capture presets, switch with one shortcut. Pause mid-take when the door knocks. Paused spans trim out cleanly.",
		tags: ["Profiles", "Pause and resume", "Multi-source"],
	},
	{
		icon: HardDriveUpload,
		title: "Local-first, Drive-shareable",
		description:
			"Recordings live on your machine until you share. Export straight to your own Google Drive. You own the file, Recast servers never see it.",
		tags: ["Local files", "Drive upload", "Own your data"],
	},
];

export const platforms = [
	{ icon: Monitor, label: "Windows", stability: "stable" as const, note: "Daily-driver stable" },
	{ icon: Apple, label: "macOS", stability: "beta" as const, note: "Active beta (12.0+)" },
	{ icon: Monitor, label: "Linux", stability: "beta" as const, note: "Active beta (Wayland + X11)" },
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

// Side-by-side comparison rows. Each row: a feature label + the
// comparison value per product. The tone of the value is set by
// the renderer in the page (primary chip vs muted text).
export const gapRows = [
	{
		feature: "Recording profiles (capture presets)",
		recast: "Built in",
		loom: "Not available",
		cap: "Limited",
	},
	{
		feature: "Hardware-accelerated export",
		recast: "Built in",
		loom: "Cloud render only",
		cap: "Partial",
	},
	{
		feature: "Files stay on your machine",
		recast: "Default",
		loom: "Cloud only",
		cap: "Local first",
	},
	{
		feature: "Share to your own storage (Drive today)",
		recast: "Built in, free",
		loom: "Not supported",
		cap: "Pro only",
	},
	{
		feature: "No account required to record",
		recast: "Never asks",
		loom: "Required",
		cap: "Required",
	},
	{
		feature: "Open source",
		recast: "GPLv3",
		loom: "Closed",
		cap: "AGPL",
	},
	{
		feature: "Per-seat pricing",
		recast: "None",
		loom: "Per seat",
		cap: "Per seat",
	},
];

// Catalog of every built-in affordance. `tag` is the small module-name
// badge that sits in the screenshot corner (Capture, Edit, Export…).
// `image` is a real screenshot when one exists; the card renders a
// tinted icon-as-hero placeholder the same width/height when null.
export const supports: Array<{
	icon: any;
	tag: string;
	title: string;
	description: string;
	image: string | null;
	href: string;
}> = [
	{
		icon: Target,
		tag: "Auto",
		title: "Smart auto-zoom",
		description: "Reads clicks and dwell, zooms toward the action. Zero keyframes.",
		image: null,
		href: "#",
	},
	{
		icon: MousePointer2,
		tag: "Cursor",
		title: "Cursor smoothing",
		description: "Velocity-aware easing, optional snap-to-target, motion damping.",
		image: null,
		href: "#",
	},
	{
		icon: VolumeX,
		tag: "Audio",
		title: "Silence detection",
		description: "Finds dead-air spans, offers one-click cuts.",
		image: null,
		href: "#",
	},
	{
		icon: Pause,
		tag: "Capture",
		title: "Pause and resume",
		description: "Pause mid-take, pick up where you left off. Paused spans trim out cleanly.",
		image: null,
		href: "#",
	},
	{
		icon: Layers,
		tag: "Capture",
		title: "Recording profiles",
		description: "Save capture presets for each context. One shortcut to switch.",
		image: null,
		href: "#",
	},
	{
		icon: Highlighter,
		tag: "Edit",
		title: "Annotations and blur",
		description: "Arrows, rectangles, text, privacy blur on the frame. Layers on the timeline.",
		image: null,
		href: "#",
	},
	{
		icon: Camera,
		tag: "Capture",
		title: "Camera bubble",
		description: "Draggable webcam with shape, border, and follow-the-cursor motion.",
		image: null,
		href: "#",
	},
	{
		icon: Layout,
		tag: "Layout",
		title: "Smart layouts",
		description: "Auto padding, gradient backgrounds, aspect framing applied as you record.",
		image: null,
		href: "#",
	},
	{
		icon: Scissors,
		tag: "Edit",
		title: "Trim, split, replace",
		description: "Lightweight editor, no hidden timeline tax.",
		image: null,
		href: "#",
	},
	{
		icon: HardDriveUpload,
		tag: "Store",
		title: "Drive uploads",
		description: "OAuth scoped to files Recast creates. Your account, your storage bill.",
		image: null,
		href: "#",
	},
	{
		icon: Zap,
		tag: "Export",
		title: "Hardware-encoded export",
		description: "NVENC, AMD, and Intel where available. Seconds, not minutes.",
		image: null,
		href: "#",
	},
	{
		icon: Cpu,
		tag: "Capture",
		title: "Native capture",
		description: "Platform APIs end to end. ScreenCaptureKit on macOS, Wayland-native on Linux.",
		image: null,
		href: "#",
	},
	{
		icon: Crop,
		tag: "Capture",
		title: "Region and window",
		description: "Capture a window, region, or full screen. Hot-swap mid-take.",
		image: null,
		href: "#",
	},
	{
		icon: FileBox,
		tag: "Files",
		title: ".recast project files",
		description: "Re-editable artifacts that travel with your repo.",
		image: null,
		href: "#",
	},
	{
		icon: WifiOff,
		tag: "Offline",
		title: "Offline first",
		description: "Recordings and exports stay on your machine.",
		image: null,
		href: "#",
	},
	{
		icon: HardDrive,
		tag: "Privacy",
		title: "No telemetry",
		description: "No phone-home. Only contacts servers when you opt in.",
		image: null,
		href: "#",
	},
	{
		icon: Keyboard,
		tag: "UX",
		title: "Shortcut-first",
		description: "Every essential action is one keystroke away.",
		image: null,
		href: "#",
	},
	{
		icon: GithubBrand,
		tag: "OSS",
		title: "GPLv3 open source",
		description: "Source on GitHub. Dual licensing for closed-source redistribution.",
		image: null,
		href: "#",
	},
];

export const verbs = ["records.", "polishes.", "shares.", "ships."];