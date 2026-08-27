<script lang="ts">
import { afterNavigate, replaceState } from "$app/navigation";
import { page } from "$app/state";
import SectionCard from "$components/layout/SectionCard.svelte";
import SettingsRow from "$components/layout/SettingsRow.svelte";
import StudioPage from "$components/layout/StudioPage.svelte";
import Logo from "$components/logo.svelte";
import RecastMark from "$components/recast-mark.svelte";
import CloudEndpoint from "$components/settings/CloudEndpoint.svelte";
import CloudSignIn from "$components/settings/CloudSignIn.svelte";
import DeviceCapabilities from "$components/settings/DeviceCapabilities.svelte";
import DiagnosticsPanel from "$components/settings/DiagnosticsPanel.svelte";
import GoogleDriveConnection from "$components/settings/GoogleDriveConnection.svelte";
import RemoteEndpoints from "$components/settings/RemoteEndpoints.svelte";
import { config } from "$constants/app";
import { syncConsent } from "$lib/analytics/client";
import {
	type CliInstallStatus,
	cliInstallStatus,
	getCliAutoInstall,
	getCloseToTray,
	getDisplays,
	getHidePanelFromCapture,
	getLastSource,
	getOutputDir,
	getWindowTransparency,
	installCli,
	setCliAutoInstall,
	setCloseToTray,
	setHidePanelFromCapture,
	setOutputDir,
	setWindowTransparency,
	uninstallCli,
} from "$lib/ipc";
import { desktopConsent } from "$lib/stores/consent.svelte";
import { LAYOUT_MODES, type LayoutMode, layoutMode } from "$lib/stores/layout-mode.svelte";
import { profilesStore } from "$lib/stores/profiles.svelte";
import { type CountdownSeconds, recordingCountdown } from "$lib/stores/recording-countdown.svelte";
import { BACKDROP_CHANGED_EVENT } from "$lib/windowBackdrop";
import {
	loadRecordingFps,
	loadRecordingQuality,
	persistRecordingFps,
	persistRecordingQuality,
	type RecordingQuality,
} from "@recast/editor/lib/profiles";
import {
	type ExperimentalFlag,
	experimentalStore,
	FLAG_META,
} from "@recast/editor/stores/experimental.svelte";
import type { IconComponent } from "@recast/icons";
import {
	ArrowUpRight,
	BrandGoogleDrive,
	Cloud,
	Cpu,
	EyeOff,
	FlaskConical,
	FolderOpen,
	Globe,
	HardDrive,
	Info,
	Monitor,
	MonitorCog,
	Moon,
	Palette,
	Pencil,
	Server,
	Settings as SettingsIcon,
	Shield,
	SlidersHorizontal as SlidersIcon,
	Sparkles,
	Sun,
	Terminal,
	Timer,
	Video,
	Wrench,
} from "@recast/icons";
import { GithubBrand } from "@recast/ui/brand-icons";
import { Button } from "@recast/ui/button";
import { safeStorage } from "@recast/ui/persisted-state";
import { Segmented, type SegmentedOption } from "@recast/ui/segmented";
import { toast } from "@recast/ui/sonner";
import { Switch } from "@recast/ui/switch";
import * as Tabs from "@recast/ui/tabs";
import { setMode } from "@recast/ui/theme";
import { emit, listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { platform } from "@tauri-apps/plugin-os";
import { onMount, untrack } from "svelte";
import {
	DEFAULT_SETTINGS_TAB,
	parseSettingsTab,
	SETTINGS_TAB_PARAM,
	type SettingsTab,
} from "./settings-tabs";
import { clampFps, computeFpsOptions, fpsToStored, resolveMaxRefresh } from "./settings.logic";

type Theme = "light" | "dark" | "system";
type EditorBehavior = "navigate" | "new-window";
// Experimental + About + device/diagnostics collapse into one "Advanced"
// section. Low-frequency, expert-facing config kept out of the main tabs.
// The tab list itself lives in `settings-tabs.ts`, so links elsewhere in the app
// can target a tab without importing this page.

let outputDir = $state("");
let currentTheme = $state<Theme>("system");
let editorWindow = $state<EditorBehavior>("navigate");
let countdown = $state<CountdownSeconds>(3);
let closeToTray = $state(true);
let windowTransparency = $state(false);
let hidePanelFromCapture = $state(true);
// Content protection is a compile-time no-op on Linux (tao gates it to
// macOS+Windows; X11/Wayland expose no per-window capture-exclusion API), so
// the toggle is shown disabled there rather than pretending it does anything.
const isLinux = platform() === "linux";
// Global recording prefs, read by the recording panel via shared localStorage.
let recordingQuality = $state<RecordingQuality>("auto");
let recordingFps = $state<number>(60);

let maxRefreshHz = $state(60);
const settingsNav = [
	{ value: "general", label: "General", icon: SettingsIcon },
	{ value: "recording", label: "Recording", icon: Video },
	{ value: "cloud", label: "Cloud", icon: Cloud },
	{ value: "diagnostics", label: "Diagnostics", icon: Cpu },
	{ value: "advanced", label: "Advanced", icon: Wrench },
] as const;

let activeTab = $state<SettingsTab>(DEFAULT_SETTINGS_TAB);
// `recast` command-line tool PATH state. null until the first probe.
let cliStatus = $state<CliInstallStatus | null>(null);
let cliBusy = $state(false);
let cliAutoInstall = $state(true);

onMount(() => {
	fetchSettings();
	void refreshCliStatus();
	profilesStore.hydrate();
	// `mode-watcher-mode` is owned by mode-watcher; we only read it to reflect
	// the current choice in the radio group.
	currentTheme = safeStorage.get<Theme>("mode-watcher-mode", currentTheme);
	editorWindow = safeStorage.get<EditorBehavior>("recast-editor-window", editorWindow);
	countdown = recordingCountdown.value;
	recordingQuality = loadRecordingQuality();
	recordingFps = loadRecordingFps() ?? 60;
	// Gate fps options by the refresh of the display that'll actually be
	// recorded (the last-selected source); re-sync when the source changes.
	void syncMaxRefresh();
	const unlistenSource = listen("source-selected", () => void syncMaxRefresh());
	return () => {
		unlistenSource.then((fn) => fn());
	};
});

// --- Tab ⇄ URL ---
// Reader first, so a deep-linked `?tab=` beats the default on the first flush.
// Each effect reads only its own source and bails when the two already agree.
$effect(() => {
	const fromUrl = parseSettingsTab(page.url.searchParams.get(SETTINGS_TAB_PARAM));
	if (fromUrl && fromUrl !== untrack(() => activeTab)) activeTab = fromUrl;
});

// `replaceState` throws until the router has booted, and effects run during
// hydration, which is earlier than that.
let routerReady = $state(false);
afterNavigate(() => {
	routerReady = true;
});

$effect(() => {
	const tab = activeTab;
	if (!routerReady) return;
	const url = untrack(() => new URL(page.url));
	if (url.searchParams.get(SETTINGS_TAB_PARAM) === tab) return;
	url.searchParams.set(SETTINGS_TAB_PARAM, tab);
	// replaceState, not goto: the open tab is view state, and one history entry
	// per tab click would make Back mean "previous tab".
	replaceState(
		url,
		untrack(() => page.state),
	);
});

/** Selected monitor's refresh when a monitor is the active source, else the
 *  highest attached display (windows/regions don't pin one). Falls back to 60. */
async function syncMaxRefresh() {
	try {
		const [displays, last] = await Promise.all([getDisplays(), getLastSource()]);
		maxRefreshHz = resolveMaxRefresh(displays, last);
	} catch {
		maxRefreshHz = 60;
	}
}

function updateRecordingQuality(value: RecordingQuality) {
	recordingQuality = value;
	persistRecordingQuality(value);
}

function updateRecordingFps(value: number) {
	recordingFps = value;
	persistRecordingFps(fpsToStored(value));
}

const fpsOptions = $derived(computeFpsOptions(maxRefreshHz));

// The stored preference is never mutated, so switching back to a high-refresh
// display restores it.
const effectiveFps = $derived(clampFps(recordingFps, fpsOptions));

const recordingQualityOptions: {
	value: RecordingQuality;
	label: string;
	desc: string;
}[] = [
	{
		value: "auto",
		label: "Auto",
		desc: "Best quality your hardware can record in real time.",
	},
	{
		value: "balanced",
		label: "Balanced",
		desc: "Fast, low CPU/GPU load. Use on weak machines.",
	},
	{
		value: "high",
		label: "High",
		desc: "Sharper detail. Slightly more load.",
	},
	{
		value: "pristine",
		label: "Pristine",
		desc: "Near-lossless. Needs a strong GPU.",
	},
];

function toggleProfilesEnabled() {
	const next = !profilesStore.enabled;
	profilesStore.setEnabled(next);
	toast.success(next ? "Profiles enabled" : "Profiles disabled");
}

function toggleExperimental(key: ExperimentalFlag, label: string) {
	const next = !experimentalStore.isEnabled(key);
	experimentalStore.setEnabled(key, next);
	toast.success(next ? `${label} enabled` : `${label} disabled`);
}

function toggleProductAnalytics() {
	const next = !desktopConsent.product;
	desktopConsent.setProduct(next);
	syncConsent();
	toast.success(next ? "Usage analytics enabled" : "Usage analytics disabled");
}

function toggleCrashReports() {
	const next = !desktopConsent.errors;
	desktopConsent.setErrors(next);
	syncConsent();
	toast.success(next ? "Crash reports enabled" : "Crash reports disabled");
}

async function fetchSettings() {
	try {
		outputDir = await getOutputDir();
	} catch (e) {
		toast.error(`Could not load settings: ${e}`);
	}
	try {
		closeToTray = await getCloseToTray();
	} catch {
		// Pre-tray builds or non-Tauri preview, so leave the default and let
		// the UI render the optimistic value.
	}
	try {
		windowTransparency = await getWindowTransparency();
	} catch {
		// Leave the default off.
	}
	try {
		hidePanelFromCapture = await getHidePanelFromCapture();
	} catch {
		// Older builds or non-Tauri preview, so keep the optimistic default.
	}
	try {
		cliAutoInstall = await getCliAutoInstall();
	} catch {
		// Optimistic default (true) is fine; settings just don't reflect
		// an explicit off toggle if the command isn't available.
	}
}

async function toggleCloseToTray() {
	const next = !closeToTray;
	closeToTray = next;
	try {
		await setCloseToTray(next);
	} catch (e) {
		// Roll back on failure so the UI mirrors the actual persisted state.
		closeToTray = !next;
		toast.error(`Could not update setting: ${e}`);
	}
}

async function toggleWindowTransparency() {
	const next = !windowTransparency;
	windowTransparency = next;
	try {
		await setWindowTransparency(next);
		// Every open window re-applies its backdrop off this broadcast.
		await emit(BACKDROP_CHANGED_EVENT, next);
	} catch (e) {
		windowTransparency = !next;
		toast.error(`Could not update setting: ${e}`);
	}
}

async function toggleHidePanelFromCapture() {
	const next = !hidePanelFromCapture;
	hidePanelFromCapture = next;
	try {
		await setHidePanelFromCapture(next);
	} catch (e) {
		hidePanelFromCapture = !next;
		toast.error(`Could not update setting: ${e}`);
	}
}

async function refreshCliStatus() {
	try {
		cliStatus = await cliInstallStatus();
	} catch {
		// Non-Tauri preview or an older build without the command.
		cliStatus = null;
	}
}

async function toggleCliInstall() {
	cliBusy = true;
	try {
		const message = cliStatus?.onPath ? await uninstallCli() : await installCli();
		toast.success(message);
		await refreshCliStatus();
	} catch (e) {
		toast.error(`Could not update the command line tool: ${e}`);
	} finally {
		cliBusy = false;
	}
}

async function toggleCliAutoInstall() {
	const next = !cliAutoInstall;
	cliAutoInstall = next;
	try {
		await setCliAutoInstall(next);
	} catch (e) {
		cliAutoInstall = !next;
		toast.error(`Could not update auto-install setting: ${e}`);
	}
}

function updateTheme(theme: Theme) {
	setMode(theme);
	currentTheme = theme;
}

function updateEditorWindow(value: EditorBehavior) {
	editorWindow = value;
	safeStorage.set("recast-editor-window", value);
}

function updateCountdown(value: CountdownSeconds) {
	countdown = value;
	recordingCountdown.set(value);
}

const countdownOptions: { value: CountdownSeconds; label: string }[] = [
	{ value: 0, label: "Off" },
	{ value: 3, label: "3s" },
	{ value: 5, label: "5s" },
	{ value: 10, label: "10s" },
];

async function pickDirectory() {
	const { open } = await import("@tauri-apps/plugin-dialog");
	const selected = await open({
		directory: true,
		multiple: false,
		title: "Select Recording Directory",
	});
	if (selected && typeof selected === "string") {
		try {
			await setOutputDir(selected);
			outputDir = selected;
			toast.success("Output directory updated");
		} catch (e) {
			toast.error(`Could not set directory: ${e}`);
		}
	}
}

const themes: { value: Theme; label: string; icon: IconComponent }[] = [
	{ value: "light", label: "Light", icon: Sun },
	{ value: "dark", label: "Dark", icon: Moon },
	{ value: "system", label: "System", icon: Monitor },
];

// Segmented-control option lists, derived from the tables above so labels
// stay in one place. Values are strings (Segmented is string-keyed); numeric
// settings parse back on change.
const themeSegments: SegmentedOption<Theme>[] = themes.map((t) => ({
	value: t.value,
	label: t.label,
}));
const layoutSegments: SegmentedOption<LayoutMode>[] = LAYOUT_MODES.map((m) => ({
	value: m.value,
	label: m.label,
}));
const countdownSegments: SegmentedOption<string>[] = countdownOptions.map((o) => ({
	value: String(o.value),
	label: o.label,
}));
const qualitySegments: SegmentedOption<RecordingQuality>[] = recordingQualityOptions.map((o) => ({
	value: o.value,
	label: o.label,
}));
const fpsSegments = $derived(
	fpsOptions.map((rate) => ({ value: String(rate), label: String(rate) })),
);
const editorSegments: SegmentedOption<EditorBehavior>[] = [
	{ value: "navigate", label: "Navigate" },
	{ value: "new-window", label: "New window" },
];
</script>

<StudioPage
  title="Settings"
  subtitle="Tune appearance, storage and editor defaults. Changes save instantly."
>
  <div class="mx-auto flex w-full min-w-0 max-w-5xl flex-col gap-6">
    <Tabs.Root
      value={activeTab}
      onValueChange={(v) => (activeTab = v as SettingsTab)}
      orientation="vertical"
      class="flex w-full flex-col gap-6 sm:flex-row sm:items-start sm:gap-8"
    >
      <Tabs.List
        variant="pill"
        class="flex shrink-0 flex-row gap-1 overflow-x-auto no-scrollbar sm:sticky sm:top-1 sm:w-48 sm:flex-col sm:gap-0.5 sm:overflow-visible"
      >
        {#each settingsNav as tab (tab.value)}
          {@const Icon = tab.icon}
          <Tabs.Trigger
            value={tab.value}
            class="w-full shrink-0 justify-start gap-2.5 rounded-lg px-3 py-2 text-[12.5px] font-medium transition-colors duration-150"
          >
            <Icon class="size-4 shrink-0" />
            {tab.label}
          </Tabs.Trigger>
        {/each}
      </Tabs.List>

      <Tabs.Content value="general" class="flex min-w-0 flex-1 flex-col gap-8">
        <SectionCard
          id="settings-appearance"
          label="Appearance"
          description="How Recast looks and how the window is arranged."
        >
          {#snippet icon()}
            <Palette class="size-4 text-muted-foreground" />
          {/snippet}
          <SettingsRow
            label="Theme"
            description={currentTheme === "system"
              ? "Following your OS preference."
              : `Locked to ${currentTheme} mode.`}
          >
            <Segmented
              options={themeSegments}
              value={currentTheme}
              onValueChange={updateTheme}
              fill={false}
              aria-label="Theme"
            />
          </SettingsRow>
          <SettingsRow
            label="Window chrome"
            description={LAYOUT_MODES.find(
              (m) => m.value === layoutMode.current,
            )?.hint}
          >
            <Segmented
              options={layoutSegments}
              value={layoutMode.current}
              onValueChange={(v) => (layoutMode.current = v)}
              fill={false}
              aria-label="Window chrome layout"
            />
          </SettingsRow>
          <SettingsRow
            label="Window transparency"
            description={isLinux
              ? "Not available on Linux."
              : windowTransparency
                ? "The window uses a translucent system backdrop (Mica on Windows 11, vibrancy on macOS). Solid on Windows 10."
                : "The window uses a solid background."}
          >
            <Switch
              checked={!isLinux && windowTransparency}
              disabled={isLinux}
              onCheckedChange={() => toggleWindowTransparency()}
              aria-label="Window transparency"
            />
          </SettingsRow>
        </SectionCard>

        <SectionCard
          id="settings-editor"
          label="Editor"
          description="Behavior when you open a recording."
        >
          {#snippet icon()}
            <Pencil class="size-4 text-muted-foreground" />
          {/snippet}
          <SettingsRow
            label="Window behavior"
            description="Replace the current view or pop the editor into its own window."
          >
            <Segmented
              options={editorSegments}
              value={editorWindow}
              onValueChange={updateEditorWindow}
              fill={false}
              aria-label="Window behavior"
            />
          </SettingsRow>
        </SectionCard>

        <SectionCard
          id="settings-storage"
          label="Storage"
          description="Where Recast keeps your recordings."
        >
          {#snippet icon()}
            <HardDrive class="size-4 text-muted-foreground" />
          {/snippet}
          <SettingsRow
            label="Output directory"
            description="New recordings save here. Existing files stay where they are."
            stacked
          >
            <div
              class="flex h-9 min-w-0 flex-1 items-center gap-2 rounded-lg border border-border/40 bg-background/60 px-3 font-mono text-[11px] text-muted-foreground"
              title={outputDir || "Default temporary directory"}
            >
              <FolderOpen class="size-3.5 shrink-0 text-muted-foreground/70" />
              <span class="truncate">
                {outputDir || "Default temporary directory"}
              </span>
            </div>
            <Button
              variant="secondary"
              size="sm"
              class="h-9 shrink-0 gap-1.5"
              onclick={pickDirectory}
            >
              <FolderOpen class="size-3.5" />
              Change
            </Button>
          </SettingsRow>
        </SectionCard>

        <SectionCard
          id="settings-system"
          label="System"
          description="Behavior when you close the main window."
        >
          {#snippet icon()}
            <MonitorCog class="size-4 text-muted-foreground" />
          {/snippet}
          <SettingsRow
            label="Minimize to tray on close"
            description={closeToTray
              ? "Closing the window hides Recast to the system tray."
              : "Closing the window quits Recast immediately."}
          >
            <Switch
              checked={closeToTray}
              onCheckedChange={() => toggleCloseToTray()}
              aria-label="Minimize to tray on close"
            />
          </SettingsRow>
        </SectionCard>

        <!-- Two locally-stored opt-ins: usage analytics (default off) and
                   crash reports (default on, PII-scrubbed). -->
        <SectionCard
          id="settings-privacy"
          label="Privacy & Telemetry"
          description="Recast is offline-first, so your recordings never leave this machine. These control anonymous diagnostics only."
        >
          {#snippet icon()}
            <Shield class="size-4 text-muted-foreground" />
          {/snippet}
          <SettingsRow
            label="Share anonymous usage analytics"
            description="Which features you use, so we know what to improve. Off by default. Nothing is sent unless you turn this on."
          >
            <Switch
              checked={desktopConsent.product}
              onCheckedChange={() => toggleProductAnalytics()}
              aria-label="Share anonymous usage analytics"
            />
          </SettingsRow>
          <SettingsRow
            label="Send anonymous crash reports"
            description="Scrubbed error details when something breaks, with no file names or paths. On by default."
          >
            <Switch
              checked={desktopConsent.errors}
              onCheckedChange={() => toggleCrashReports()}
              aria-label="Send anonymous crash reports"
            />
          </SettingsRow>
        </SectionCard>
      </Tabs.Content>

      <Tabs.Content
        value="recording"
        class="flex min-w-0 flex-1 flex-col gap-8"
      >
        <!-- Read by the recording panel via shared localStorage; profiles
                   can override it per-profile. -->
        <SectionCard
          id="settings-countdown"
          label="Countdown"
          description="Wait a beat before capture starts, so you can switch windows."
        >
          {#snippet icon()}
            <Timer class="size-4 text-muted-foreground" />
          {/snippet}
          <SettingsRow
            label="Countdown before recording"
            description={countdown === 0
              ? "Recording starts immediately."
              : `A ${countdown}-second countdown shows in the panel first.`}
          >
            <Segmented
              options={countdownSegments}
              value={String(countdown)}
              onValueChange={(v) =>
                updateCountdown(Number(v) as CountdownSeconds)}
              fill={false}
              aria-label="Countdown before recording"
            />
          </SettingsRow>
        </SectionCard>

        <!-- Higher tiers raise fidelity at the cost of encode headroom; if
                   the GPU can't keep up the result is judder, never desync. -->
        <SectionCard
          id="settings-capture-quality"
          label="Capture quality"
          description="How crisp the recorded master is. The editor re-encodes on export, but detail lost here can't be recovered later."
        >
          {#snippet icon()}
            <Sparkles class="size-4 text-muted-foreground" />
          {/snippet}
          <SettingsRow
            label="Recording quality"
            description={recordingQualityOptions.find(
              (o) => o.value === recordingQuality,
            )?.desc}
          >
            <Segmented
              options={qualitySegments}
              value={recordingQuality}
              onValueChange={updateRecordingQuality}
              fill={false}
              aria-label="Recording quality"
            />
          </SettingsRow>
        </SectionCard>

        <!-- Options gated by display refresh: capturing above it only
                   duplicates frames. 60 is always available. -->
        <SectionCard
          id="settings-capture-fps"
          label="Frame rate"
          description={fpsOptions.length > 1
            ? `Higher frame rates capture smoother motion. Your display supports up to ${maxRefreshHz} Hz.`
            : `Smoother motion needs a higher-refresh display. Yours runs at ${maxRefreshHz} Hz, so 60 fps is the max useful rate.`}
        >
          {#snippet icon()}
            <Video class="size-4 text-muted-foreground" />
          {/snippet}
          <SettingsRow
            label="Recording frame rate"
            description={recordingFps > effectiveFps
              ? `Set to ${recordingFps} fps, but this display runs at ${maxRefreshHz} Hz, so capture uses ${effectiveFps} fps here.`
              : `${recordingFps} fps. Bigger files and more encode load at higher rates.`}
          >
            <Segmented
              options={fpsSegments}
              value={String(effectiveFps)}
              onValueChange={(v) => updateRecordingFps(Number(v))}
              fill={false}
              aria-label="Recording frame rate"
            />
          </SettingsRow>
        </SectionCard>

        <SectionCard
          id="settings-panel-capture"
          label="Recording panel"
          description="Whether Recast's own floating controls show up in the video."
        >
          {#snippet icon()}
            <EyeOff class="size-4 text-muted-foreground" />
          {/snippet}
          <SettingsRow
            label="Hide recording panel from captures"
            description={isLinux
              ? "Not available on Linux. X11 and Wayland provide no way for an app to exclude its own window from screen capture."
              : hidePanelFromCapture
                ? "The floating Recast panel is kept out of your recordings, including one that's already open."
                : "The floating Recast panel appears in your recordings like any other window."}
          >
            <Switch
              checked={!isLinux && hidePanelFromCapture}
              disabled={isLinux}
              onCheckedChange={() => toggleHidePanelFromCapture()}
              aria-label="Hide recording panel from captures"
            />
          </SettingsRow>
        </SectionCard>

        <SectionCard
          id="settings-profiles"
          label="Recording profiles"
          description="Save preset combinations of audio, mic, and camera."
        >
          {#snippet icon()}
            <SlidersIcon class="size-4 text-muted-foreground" />
          {/snippet}
          <SettingsRow
            label="Use profile system"
            description={profilesStore.enabled
              ? "Recording panel auto-applies the default profile and shows a switcher."
              : "Recording panel resets to manual toggles every launch."}
          >
            <Switch
              checked={profilesStore.enabled}
              onCheckedChange={() => toggleProfilesEnabled()}
              aria-label="Use profile system"
            />
          </SettingsRow>
          {#if profilesStore.enabled}
            <SettingsRow
              label="Manage profiles"
              description={profilesStore.profiles.length === 0
                ? "No profiles yet."
                : profilesStore.profiles.length === 1
                  ? "1 profile saved."
                  : `${profilesStore.profiles.length} profiles saved.`}
            >
              <Button
                href="/profiles"
                variant="secondary"
                size="sm"
                class="h-8 gap-1.5"
              >
                <SlidersIcon class="size-3.5" />
                <span class="text-[11.5px]">Open profiles</span>
              </Button>
            </SettingsRow>
          {/if}
        </SectionCard>
      </Tabs.Content>

      <Tabs.Content value="cloud" class="flex min-w-0 flex-1 flex-col gap-8">
      
        <SectionCard
          id="settings-cloud"
          label="Recast Cloud"
          description="Share recordings as Loom-style links, layered on top of your local recordings."
        >
          {#snippet icon()}
            <RecastMark class="size-4 text-muted-foreground" />
          {/snippet}
          <CloudSignIn />
        </SectionCard>
        {#if experimentalStore.isEnabled("selfHosting")}
          <section id="settings-cloud-endpoint" class="flex flex-col gap-3">
            <div class="px-1">
              <h2
                class="flex items-center gap-1.5 text-[13px] font-semibold tracking-tight text-foreground"
              >
                <Server class="size-4 text-muted-foreground" />
                Self-hosting
                <span
                  class="inline-flex items-center gap-1 rounded-full bg-warning/12 px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wide text-warning"
                >
                  <FlaskConical class="size-2.5" />
                  Experimental
                </span>
              </h2>
              <p
                class="mt-0.5 text-[11.5px] leading-relaxed text-muted-foreground"
              >
                Run your own Recast Cloud server? Set its address here. Everyone
                else can leave this on the default.
              </p>
            </div>
            <div
              class="overflow-hidden rounded-2xl border border-border/50 bg-card/70 shadow-(--shadow-craft-inset) backdrop-blur"
            >
              <CloudEndpoint />
            </div>
          </section>
        {/if}

        <!-- Separate auth from Recast Cloud above; both are external
                   integrations that take exports off this machine. -->
        <SectionCard
          id="settings-google-drive"
          label="Google Drive"
          description="Upload exports to your own Drive. Files land in a private /Recast/ folder."
        >
          {#snippet icon()}
            <BrandGoogleDrive class="size-4 text-muted-foreground" />
          {/snippet}
          <GoogleDriveConnection />
        </SectionCard>
      </Tabs.Content>

      <Tabs.Content value="advanced" class="flex min-w-0 flex-1 flex-col gap-8">
        <SectionCard
          id="settings-experimental"
          label="Experimental"
          description="Unfinished features, off by default. Turn one on to try it; it may change or break."
        >
          {#snippet icon()}
            <FlaskConical class="size-4 text-muted-foreground" />
          {/snippet}
          {#each FLAG_META as flag (flag.key)}
            {@const on = experimentalStore.isEnabled(flag.key)}
            <SettingsRow label={flag.label} description={flag.description}>
              <Switch
                checked={on}
                onCheckedChange={() => toggleExperimental(flag.key, flag.label)}
                aria-label={flag.label}
              />
            </SettingsRow>
          {/each}
        </SectionCard>

        <!-- Gated behind `remoteTranscription`: response formats vary
                   across OpenAI-compatible servers, so this is early. -->
        {#if experimentalStore.isEnabled("remoteTranscription")}
          <section id="settings-remote-asr" class="flex flex-col gap-3">
            <div class="px-1">
              <h2
                class="flex items-center gap-1.5 text-[13px] font-semibold tracking-tight text-foreground"
              >
                <Server class="size-4 text-muted-foreground" />
                Remote transcription
                <span
                  class="inline-flex items-center gap-1 rounded-full bg-warning/12 px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wide text-warning"
                >
                  <FlaskConical class="size-2.5" />
                  Experimental
                </span>
              </h2>
              <p
                class="mt-0.5 text-[11.5px] leading-relaxed text-muted-foreground"
              >
                Transcribe captions through an OpenAI-compatible endpoint. Keys
                are stored in your OS keyring, never in the project.
              </p>
            </div>
            <div
              class="overflow-hidden rounded-2xl border border-border/50 bg-card/70 shadow-(--shadow-craft-inset) backdrop-blur"
            >
              <RemoteEndpoints />
            </div>
          </section>
        {/if}

        <!-- Power-user: exposes the same `recast` binary as a CLI so a
                   terminal or an AI agent can drive recording. -->
        <SectionCard
          id="settings-cli"
          label="Command line tool"
          description="Control Recast from a terminal or an AI agent with the recast command."
        >
          {#snippet icon()}
            <Terminal class="size-4 text-muted-foreground" />
          {/snippet}
          <SettingsRow
            label="Install the recast command"
            description={cliStatus === null
              ? "Checking availability."
              : cliStatus.onPath
                ? "Available in any terminal. Try recast --help."
                : "Not on your PATH yet. Install it to run recast from any terminal."}
          >
            <Button
              variant="secondary"
              size="sm"
              class="h-8 gap-1.5"
              disabled={cliBusy}
              onclick={() => toggleCliInstall()}
            >
              <Terminal class="size-3.5" />
              <span class="text-[11.5px]">
                {cliStatus?.onPath ? "Remove" : "Install"}
              </span>
            </Button>
          </SettingsRow>

          <SettingsRow
            label="Auto-install on first launch"
            description="When enabled, Recast puts itself on your PATH the first time the app starts. Disables future auto-attempts."
          >
            <Switch
              checked={cliAutoInstall}
              onCheckedChange={() => toggleCliAutoInstall()}
              aria-label="Auto-install on first launch"
            />
          </SettingsRow>

          {#if cliStatus?.modifiedRcFiles && cliStatus.modifiedRcFiles.length > 0}
            <div class="text-[11px] text-muted-foreground/80 px-1">
              <span class="font-medium">Modified shell config:</span>
              <span class="ml-1 inline-flex flex-wrap gap-1">
                {#each cliStatus.modifiedRcFiles as f (f)}
                  <span
                    class="rounded bg-muted/60 px-1.5 py-0.5 font-mono text-[10.5px]"
                  >
                    {f.split(/[\\/]/).pop()}
                  </span>
                {/each}
              </span>
            </div>
          {/if}
        </SectionCard>

        <SectionCard
          id="settings-about"
          label="About"
          description="Version info and where to find us."
        >
          {#snippet icon()}
            <Info class="size-4 text-muted-foreground" />
          {/snippet}
          <div class="flex flex-col gap-3 px-4 py-4">
            <div class="flex items-center gap-3">
              <div
                class="flex size-10 shrink-0 items-center justify-center rounded-xl bg-foreground/5 text-foreground ring-1 ring-inset ring-border/40"
              >
                <Logo class="size-6" />
              </div>
              <div class="min-w-0 flex-1">
                <div class="text-[13px] font-semibold text-foreground">
                  {config.appName}
                </div>
                <div class="font-mono text-[10.5px] text-muted-foreground">
                  v{config.appVersion}
                </div>
              </div>
            </div>
            <div class="flex flex-wrap gap-2">
              <Button href="/whats-new" variant="outline" size="xs">
                <Sparkles />
                <span>What's new</span>
                <ArrowUpRight class="text-muted-foreground" />
              </Button>
              <!-- `openUrl`, not `target="_blank"`: the webview silently
                         swallows a new-window request, so both of these were
                         dead buttons. -->
              <Button
                variant="outline"
                size="xs"
                onclick={() => void openUrl(config.website)}
              >
                <Globe />
                <span>Website</span>
                <ArrowUpRight class="text-muted-foreground" />
              </Button>
              <Button
                variant="outline"
                size="xs"
                onclick={() => void openUrl(config.github)}
              >
                <GithubBrand />
                <span>GitHub</span>
                <ArrowUpRight class="text-muted-foreground" />
              </Button>
            </div>
          </div>
        </SectionCard>
      </Tabs.Content>

      <Tabs.Content
        value="diagnostics"
        class="flex min-w-0 flex-1 flex-col gap-8"
      >
        <SectionCard
          id="settings-device"
          label="Device & diagnostics"
          description="System status and which video encoders this device supports."
        >
          {#snippet icon()}
            <Cpu class="size-4 text-muted-foreground" />
          {/snippet}
          <DeviceCapabilities />
        </SectionCard>

        <DiagnosticsPanel />
      </Tabs.Content>
    </Tabs.Root>
  </div>
</StudioPage>
