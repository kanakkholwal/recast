  import {
    Cpu,
    FileBox,
    HardDrive,
    MemoryStick,
    MonitorSmartphone,
    ShieldCheck,
    WifiOff,
    Zap
} from "@lucide/svelte";
import { AppleBrand, LinuxBrand, WindowsBrand } from "@recast/ui/brand-icons";
import type { PageData } from "./$types";

  let { data }: { data: PageData } = $props();

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

  export const stabilityCopy: Record<
    Stability,
    { label: string; dot: string; chip: string }
  > = {
    stable: {
      label: "Stable",
      dot: "bg-emerald-500",
      chip: "bg-emerald-500/10 text-emerald-600 ring-emerald-500/20 dark:text-emerald-400",
    },
    beta: {
      label: "Beta · expect rough edges",
      dot: "bg-amber-500",
      chip: "bg-amber-500/10 text-amber-600 ring-amber-500/20 dark:text-amber-400",
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
      minimum: "Dual-core x86_64 or arm64 @ 2.0 GHz",
      recommended: "Quad-core (8+ threads) @ 3.0 GHz or faster",
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
      minimum: "Integrated GPU or none; falls back to CPU (libx264)",
      recommended:
        "NVIDIA (NVENC, GTX 10-series+), AMD (AMF, RX 400+), or Intel iGPU (QSV, 6th-gen+)",
    },
    {
      icon: HardDrive,
      label: "Disk",
      minimum:
        "500 MB free for the app + room for recordings (~1 GB / 10 min at 1080p60)",
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
        "macOS is in beta. The smoothest path is Homebrew (step 1): one line that grabs the right build for your chip and clears Gatekeeper for you. Prefer the .dmg? Steps 2–5 cover the manual install, with one Terminal command on first launch until we're Apple-notarized.",
      steps: [
        {
          title: "Fastest: install with Homebrew",
          body: 'One line installs the right build for your Mac and clears the Gatekeeper quarantine, so there\'s no "is damaged" error, and brew keeps it updated. Prefer the short name? Run brew tap kanakkholwal/recast, then brew install --cask recast.',
          code: "brew install --cask kanakkholwal/recast/recast",
          hint: "Installed this way? You're done. Skip the manual .dmg steps below.",
        },
        {
          title: "Or download the .dmg and pick the right build",
          body: "Apple Silicon for M1/M2/M3/M4 Macs. Intel for older models. Check via   → About This Mac if you're unsure.",
        },
        {
          title: "Drag Recast into Applications",
          body: "Open the downloaded .dmg, then drag Recast.app into your Applications folder. Eject the disk image when you're done.",
        },
        {
          title: "Clear the quarantine attribute",
          body: "Open Terminal and run this once. macOS will trust Recast afterwards.",
          code: "xattr -dr com.apple.quarantine /Applications/Recast.app",
          hint: "Disappears as soon as we ship a notarized build.",
        },
        {
          title: "Grant capture permissions",
          body: "On first launch, System Settings → Privacy & Security will prompt for Screen Recording, Microphone, and Camera. Enable the ones you intend to record from.",
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
        "Windows SmartScreen flags new publishers as 'Unknown'. One click past the warning and you're in. This goes away once we sign with an EV certificate.",
      steps: [
        {
          title: "Pick the right installer",
          body: ".exe is the typical install. Use the .msi if your IT department's group policy requires MSI packages.",
        },
        {
          title: "Run the installer",
          body: "Double-click the downloaded file. If UAC asks, allow it to run.",
        },
        {
          title: "Bypass SmartScreen",
          body: "If you see 'Windows protected your PC', click More info, then Run anyway. The publisher will read as 'Unknown' until we add code signing.",
        },
        {
          title: "Finish setup",
          body: "Pick an install location and let the wizard finish. Recast launches from the Start menu, so pin it to the taskbar while you're at it.",
        },
      ],
      faqs: [
        {
          title: "Antivirus flags Recast as suspicious",
          body: "It's a false positive. Fresh unsigned binaries trip heuristic scanners until they age. Add Recast.exe to your antivirus's allowlist.",
        },
        {
          title: "Capture is empty or black",
          body: "Update your GPU drivers (NVIDIA/AMD/Intel) and make sure Recast isn't running in compatibility mode. Right-click the shortcut → Properties → Compatibility → uncheck everything.",
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
          body: "Recast uses xdg-desktop-portal for screen capture under Wayland. Most distros bundle it; if capture is empty, install the portal and the matching backend (GNOME or KDE).",
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
          body: "Recast captures via PipeWire. Make sure pipewire-pulse is installed so your default sink is exposed.",
          code: "sudo apt install pipewire-pulse",
        },
      ],
    },
  };
