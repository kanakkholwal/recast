import { goto } from "$app/navigation";
import { config } from "$constants/app";
import { getOutputDir, launchRecordingPanel, openFileLocation } from "$lib/ipc";
import type { PaletteCommand } from "$lib/stores/command-palette.svelte";
import {
	Camera,
	Download,
	ExternalLink,
	Film,
	FolderOpen,
	Github,
	Home,
	Monitor,
	Moon,
	Radio,
	Settings,
	SlidersHorizontal,
	Sparkles,
	Sun,
} from "@lucide/svelte";
import { toast } from "@recast/ui/sonner";
import { setMode } from "@recast/ui/theme";

export function buildGlobalCommands(): PaletteCommand[] {
	return [
		// Navigation
		{
			id: "nav.home",
			title: "Go to Home",
			description: "Open the dashboard",
			category: "Navigation",
			icon: Home,
			keywords: ["home", "dashboard", "start"],
			action: () => goto("/"),
		},
		{
			id: "nav.recasts",
			title: "Go to Recasts",
			description: "Browse your screen recordings",
			category: "Navigation",
			icon: Film,
			keywords: ["recordings", "videos", "library"],
			action: () => goto("/recasts"),
		},
		{
			id: "nav.exports",
			title: "Go to Exports",
			description: "Browse exported videos",
			category: "Navigation",
			icon: Download,
			keywords: ["export", "rendered", "share"],
			action: () => goto("/exports"),
		},
		{
			id: "nav.profiles",
			title: "Go to Recording Profiles",
			description: "Manage recording presets",
			category: "Navigation",
			icon: SlidersHorizontal,
			keywords: ["profiles", "presets", "config"],
			action: () => goto("/profiles"),
		},
		{
			id: "nav.settings",
			title: "Go to Settings",
			description: "Configure Recast preferences",
			category: "Navigation",
			icon: Settings,
			keywords: ["settings", "preferences", "config"],
			action: () => goto("/settings"),
		},
		{
			id: "nav.whats-new",
			title: "What's New",
			description: "See the latest release notes",
			category: "Navigation",
			icon: Sparkles,
			keywords: ["changelog", "release", "updates", "notes"],
			action: () => goto("/whats-new"),
		},

		// Recording
		{
			id: "rec.launch-panel",
			title: "Launch Recording Panel",
			description: "Open the floating recorder",
			category: "Recording",
			icon: Radio,
			keywords: ["record", "start", "panel", "capture"],
			shortcut: "⌘⇧R",
			action: () => launchRecordingPanel(),
		},
		{
			id: "rec.device-picker",
			title: "Open Device Picker",
			description: "Pick a screen or window to record",
			category: "Recording",
			icon: Monitor,
			keywords: ["display", "window", "source"],
			action: () => goto("/device-picker"),
		},
		{
			id: "rec.camera-preview",
			title: "Open Camera Preview",
			description: "Test your webcam",
			category: "Recording",
			icon: Camera,
			keywords: ["camera", "webcam", "preview"],
			action: () => goto("/camera-preview"),
		},

		// Files
		{
			id: "file.show-output",
			title: "Show Output Folder",
			description: "Reveal the recordings directory",
			category: "Files",
			icon: FolderOpen,
			keywords: ["folder", "directory", "reveal", "explorer"],
			action: async () => {
				try {
					const dir = await getOutputDir();
					await openFileLocation(dir);
				} catch (e) {
					toast.error(`Could not open output folder: ${e}`);
				}
			},
		},

		// Theme
		{
			id: "theme.light",
			title: "Set Theme: Light",
			category: "Theme",
			icon: Sun,
			keywords: ["light", "theme", "appearance"],
			action: () => setMode("light"),
		},
		{
			id: "theme.dark",
			title: "Set Theme: Dark",
			category: "Theme",
			icon: Moon,
			keywords: ["dark", "theme", "appearance"],
			action: () => setMode("dark"),
		},
		{
			id: "theme.system",
			title: "Set Theme: System",
			category: "Theme",
			icon: Monitor,
			keywords: ["system", "auto", "theme"],
			action: () => setMode("system"),
		},

		// External
		{
			id: "ext.github",
			title: "View on GitHub",
			description: "Open the source repository",
			category: "External",
			icon: Github,
			keywords: ["github", "source", "repo"],
			action: () => {
				window.open(config.github, "_blank");
			},
		},
		{
			id: "ext.website",
			title: "Open Website",
			category: "External",
			icon: ExternalLink,
			keywords: ["website", "homepage", "web"],
			action: () => {
				window.open(config.website, "_blank");
			},
		},
	];
}
