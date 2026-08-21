import {
	Cpu,
	FileBox,
	HardDrive,
	MemoryStick,
	MonitorSmartphone,
	ShieldCheck,
	WifiOff,
	Zap,
} from "@recast/icons";
import { AppleBrand, LinuxBrand, WindowsBrand } from "@recast/ui/brand-icons";

export type OS = "macOS" | "Windows" | "Linux" | "Unknown";

// Per-platform shipping confidence. Surfaces the honest state of the
// builds: Windows is the daily-driver, macOS/Linux are early ports. The
// global heads-up card below the hero plus the per-tab chip both read
// from this so the messaging stays in sync.
export type Stability = "stable" | "beta";
export const platforms: Array<{
	id: Exclude<OS, "Unknown">;
	icon: typeof AppleBrand;
	title: string;
	subtitle: string;
	stability: Stability;
}> = [
	{
		id: "macOS",
		icon: AppleBrand,
		title: "macOS",
		subtitle: "Requires macOS 12.0 or later",
		stability: "beta",
	},
	{
		id: "Windows",
		icon: WindowsBrand,
		title: "Windows",
		subtitle: "Requires Windows 10 or later",
		stability: "stable",
	},
	{
		id: "Linux",
		icon: LinuxBrand,
		title: "Linux",
		subtitle: "Debian, Ubuntu, Fedora, Arch",
		stability: "beta",
	},
];

export const stabilityCopy: Record<Stability, { label: string; dot: string; chip: string }> = {
	stable: {
		label: "Stable",
		dot: "bg-tag-green",
		chip: "bg-tag-green/12 text-tag-green",
	},
	beta: {
		label: "Beta, expect rough edges",
		dot: "bg-tag-tangerine",
		chip: "bg-tag-tangerine/12 text-tag-tangerine",
	},
};

export const ISSUES_URL = "https://github.com/kanakkholwal/recast/issues/new";

export const ships = [
	{ icon: WifiOff, label: "Offline-first", value: "Stays on disk" },
	{ icon: Zap, label: "GPU export", value: "Hardware-encoded" },
	{ icon: FileBox, label: "Open format", value: ".recast project" },
	{ icon: ShieldCheck, label: "Open source", value: "GPLv3 licensed" },
];

// System requirements. Recast probes NVENC (NVIDIA) → AMF (AMD) → QSV
// (Intel) at startup and falls back to libx264 (CPU) if none initialize.
// The "recommended" tier is what makes recording feel realtime at 1080p60;
// the "minimum" tier covers the integrated-GPU and no-GPU CPU path so
// users on older laptops know they're supported before they download.
export const systemRequirements = [
	{
		icon: Cpu,
		label: "CPU",
		minimum: "Dual-core x86_64 or arm64 at 2.0 GHz",
		recommended: "Quad-core, 8+ threads at 3.0 GHz",
	},
	{
		icon: MemoryStick,
		label: "RAM",
		minimum: "4 GB",
		recommended: "8 GB or more",
	},
	{
		icon: Zap,
		label: "GPU",
		minimum: "Integrated or none, falls back to CPU",
		recommended: "NVIDIA GTX 10-series, AMD RX 400, or Intel 6th-gen iGPU",
	},
	{
		icon: HardDrive,
		label: "Disk",
		minimum: "500 MB, plus ~1 GB per 10 min at 1080p60",
		recommended: "SSD with 10+ GB free",
	},
	{
		icon: MonitorSmartphone,
		label: "Display",
		minimum: "1280 × 720",
		recommended: "1920 × 1080 or higher",
	},
];

// Per-platform install instructions. Step `code` is a copy-paste-ready
// shell command; `hint` is small print rendered under the body.
export type InstallStep = {
	title: string;
	body: string;
	code?: string;
	hint?: string;
};
export type Faq = { title: string; body: string; code?: string };
export type PlatformGuide = { intro: string; steps: InstallStep[]; faqs: Faq[] };

export const installSteps: Record<Exclude<OS, "Unknown">, PlatformGuide> = {
	macOS: {
		intro:
			"Homebrew is the smooth path: one line, right build, Gatekeeper cleared. The .dmg route needs one Terminal command until we are notarized.",
		steps: [
			{
				title: "Fastest: install with Homebrew",
				body: "Installs the right build for your chip and clears the quarantine, so no damaged-file error.",
				code: "brew install --cask kanakkholwal/recast/recast",
				hint: "Installed this way? Skip the .dmg steps below.",
			},
			{
				title: "Or download the .dmg and pick the right build",
				body: "Apple Silicon for M1 and later, Intel for older Macs. Check under About This Mac if you are unsure.",
			},
			{
				title: "Drag Recast into Applications",
				body: "Open the .dmg, drag Recast.app into Applications, then eject the disk image.",
			},
			{
				title: "Clear the quarantine attribute",
				body: "Open Terminal and run this once. macOS will trust Recast afterwards.",
				code: "xattr -dr com.apple.quarantine /Applications/Recast.app",
				hint: "Disappears as soon as we ship a notarized build.",
			},
			{
				title: "Grant capture permissions",
				body: "First launch prompts for Screen Recording, Microphone and Camera under Privacy & Security. Enable what you record from.",
			},
		],
		faqs: [
			{
				title: "“Recast is damaged and can't be opened”",
				body: "It's not actually damaged. That's the un-notarized Gatekeeper error, and step 3 above fixes it.",
			},
			{
				title: "Permissions don't stick after enabling",
				body: "Fully quit Recast (⌘Q), toggle the permission off and on under Privacy & Security, then relaunch.",
			},
		],
	},
	Windows: {
		intro:
			"SmartScreen flags new publishers as unknown. One click past it and you are in, and it goes once we sign with an EV certificate.",
		steps: [
			{
				title: "Pick the right installer",
				body: ".exe is the normal install. Use the .msi if group policy requires MSI packages.",
			},
			{
				title: "Run the installer",
				body: "Double-click the downloaded file. If UAC asks, allow it to run.",
			},
			{
				title: "Bypass SmartScreen",
				body: "Click More info, then Run anyway. The publisher reads as unknown until we add code signing.",
			},
			{
				title: "Finish setup",
				body: "Pick a location and finish the wizard. Recast launches from the Start menu.",
			},
		],
		faqs: [
			{
				title: "Antivirus flags Recast as suspicious",
				body: "A false positive: fresh unsigned binaries trip heuristic scanners. Allowlist Recast.exe.",
			},
			{
				title: "Capture is empty or black",
				body: "Update your GPU drivers, then uncheck everything under the shortcut's Compatibility tab.",
			},
		],
	},
	Linux: {
		intro:
			"Three packages cover most distros. Pick by your package manager; the AppImage works on anything.",
		steps: [
			{
				title: "Pick your package",
				body: "AppImage = portable (any distro, no install). .deb = Debian, Ubuntu, Mint. .rpm = Fedora, RHEL, openSUSE.",
			},
			{
				title: "AppImage: mark executable & run",
				body: "Give it execute permission, then double-click or run from the terminal.",
				code: "chmod +x Recast-*.AppImage\n./Recast-*.AppImage",
				hint: "Some distros need libfuse2: sudo apt install libfuse2.",
			},
			{
				title: ".deb: install with apt",
				body: "apt resolves any missing dependencies for you.",
				code: "sudo apt install ./recast_*.deb",
			},
			{
				title: ".rpm: install with dnf",
				body: "Use zypper on openSUSE: sudo zypper install ./recast-*.rpm.",
				code: "sudo dnf install ./recast-*.rpm",
			},
			{
				title: "Wayland: enable the portal",
				body: "Wayland capture runs through xdg-desktop-portal. If capture is empty, install it with your desktop's backend.",
				code: "sudo apt install xdg-desktop-portal xdg-desktop-portal-gnome",
			},
		],
		faqs: [
			{
				title: "AppImage won't launch",
				body: "Missing FUSE library on newer Ubuntu / Debian.",
				code: "sudo apt install libfuse2",
			},
			{
				title: "No audio device shows up",
				body: "Capture runs through PipeWire. Install pipewire-pulse so your default sink is exposed.",
				code: "sudo apt install pipewire-pulse",
			},
		],
	},
};
