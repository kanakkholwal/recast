<script lang="ts">
import "@fontsource-variable/google-sans";
import { TooltipProvider } from "@recast/ui/tooltip";
import "../app.css";
// Loaded once globally so any route that mounts <RecastPlayer> gets its styling.
import "@recast/player/styles.css";

import { setAgentSessionDriver, setEditorHostHooks, setLogSink } from "@recast/editor";
import { setEditorServicesForApp } from "@recast/editor/lib/editor/services";
import { onNavigate } from "$app/navigation";
import { navigating, page } from "$app/state";
import { handleDeepLink } from "$lib/deepLink";
import { tauriAgentSessionDriver } from "$lib/editor/agent-session.tauri";
import { tauriEditorServices } from "$lib/editor/services.tauri";
import { launchRecordingPanel, takePendingNewRecording, takePendingOpenFile } from "$lib/ipc";
import { openProjectFromExternalPath } from "$lib/openProject";
import { chordLabel, registerShortcutHandlers } from "$lib/shortcuts/registry.svelte";
import { exportActivity } from "$lib/stores/exportActivity.svelte";
import { updater } from "$lib/stores/updater.svelte";
import { applyWindowBackdrop, BACKDROP_CHANGED_EVENT } from "$lib/windowBackdrop";
import { workerHost } from "$lib/workers";

let { children } = $props();

// App-scoped, not editor-scoped: the export queue and asset helpers run outside any editor component.
setEditorServicesForApp(tauriEditorServices);
// The editor package defaults these to no-ops; the real ones keep telemetry, chords and the export pause working.
setEditorHostHooks({
	analytics,
	workers: workerHost,
	shortcuts: { chordLabel, registerShortcutHandlers },
	exportActivity,
});
// Without a driver installed the listener stays idle, which is the web build's behaviour.
setAgentSessionDriver(tauriAgentSessionDriver);
setLogSink(log);

// First-run privacy prompt, shown once in the main window only.
let showFirstRun = $state(false);

// Overlay windows are skipped; the main window owns app_opened, identify and the first-run prompt.
onMount(() => {
	if (isTransparentRoute) return;

	let cancelled = false;
	let unlistenAuth: (() => void) | undefined;

	const setup = async () => {
		const { getCurrentWebviewWindow } = await import("@tauri-apps/api/webviewWindow");
		if (getCurrentWebviewWindow().label !== "main") return;

		// `app_opened` is a no-op unless the user opted into product analytics.
		try {
			const { platform } = await import("@tauri-apps/plugin-os");
			analytics.register({ os: platform() });
		} catch {
			// Non-Tauri preview, so leave os unset.
		}
		analytics.capture("app_opened");

		if (!desktopConsent.hasSeenFirstRun) showFirstRun = true;

		// Alias anonymous events to the cloud account on sign-in; a missing `userId` leaves only the install id.
		const unlisten = await listen<{ userId?: string | null }>("auth:signed-in", ({ payload }) => {
			if (payload?.userId) analytics.identify(payload.userId);
			analytics.capture("cloud_connected");
		});
		if (cancelled) unlisten();
		else unlistenAuth = unlisten;
	};
	void setup();

	// Global JS error capture → scrubbed $exception (default-on errors consent).
	const onError = (e: ErrorEvent) =>
		analytics.captureError(e.error ?? e.message, {
			source: "desktop",
			route: page.url.pathname,
		});
	const onRejection = (e: PromiseRejectionEvent) =>
		analytics.captureError(e.reason, {
			source: "desktop",
			route: page.url.pathname,
		});
	window.addEventListener("error", onError);
	window.addEventListener("unhandledrejection", onRejection);

	return () => {
		cancelled = true;
		unlistenAuth?.();
		window.removeEventListener("error", onError);
		window.removeEventListener("unhandledrejection", onRejection);
	};
});

import { initAssets } from "@recast/editor/lib/assets";
import { initExtensions } from "@recast/editor/lib/extensions";
import { NavProgress } from "@recast/ui/nav-progress";
import { safeStorage } from "@recast/ui/persisted-state";
import { Toaster, toast } from "@recast/ui/sonner";
import { ModeWatcher, setMode } from "@recast/ui/theme";
import { listen } from "@tauri-apps/api/event";
import { onMount, tick } from "svelte";
import FirstRunConsent from "$components/FirstRunConsent.svelte";
import CommandPaletteHost from "$components/layout/CommandPaletteHost.svelte";
import ShortcutsDialog from "$components/layout/ShortcutsDialog.svelte";
import { analytics } from "$lib/analytics/client";
import { log } from "$lib/logger";
import { isOverlayRoute } from "$lib/runtime/overlay-routes";
import { getTauriTheme, isTauriApp } from "$lib/runtime/tauri";
import { dispatchShortcut } from "$lib/shortcuts/registry.svelte";
import { desktopConsent } from "$lib/stores/consent.svelte";

const isTransparentRoute = $derived(isOverlayRoute(page.url.pathname));

// Transparent-route windows are too narrow for a Sonner card, so they emit `ui:toast` for the main Toaster.
type UiToastPayload = {
	level: "error" | "warning" | "info" | "success";
	message: string;
	duration?: number;
};
onMount(() => {
	if (isTransparentRoute) return;
	const unlisten = listen<UiToastPayload>("ui:toast", ({ payload }) => {
		const opts = payload.duration ? { duration: payload.duration } : undefined;
		switch (payload.level) {
			case "error":
				toast.error(payload.message, opts);
				break;
			case "warning":
				toast.warning(payload.message, opts);
				break;
			case "success":
				toast.success(payload.message, opts);
				break;
			default:
				toast.info(payload.message, opts);
		}
	});
	return () => {
		unlisten.then((fn) => fn());
	};
});

// Main-window handlers cover tray actions when no recording is active; panel and overlay routes own their own.
onMount(() => {
	if (isTransparentRoute) return;
	const offToggle = listen("tray:record-toggle", async () => {
		// An open panel owns the toggle, so don't steal focus mid-stop. The label must match launchRecordingPanel() in ipc.ts.
		const { getAllWebviewWindows } = await import("@tauri-apps/api/webviewWindow");
		const all = await getAllWebviewWindows();
		const hasPanel = all.some((w) => w.label === "recording-panel");
		if (hasPanel) return;
		void launchRecordingPanel();
	});
	const offCheckUpdates = listen("updater:check-from-tray", () => {
		void updater.checkNow();
	});
	return () => {
		void offToggle.then((fn) => fn());
		void offCheckUpdates.then((fn) => fn());
	};
});

// Cold start drains argv from AppState, warm start gets `app://open-recast`; both spawn a fresh editor window, main-window only.
onMount(() => {
	if (isTransparentRoute) return;
	let cancelled = false;
	let unlistenFn: (() => void) | undefined;
	let unlistenDeepLink: (() => void) | undefined;
	let unlistenPanelFn: (() => void) | undefined;

	const setup = async () => {
		const { getCurrentWebviewWindow } = await import("@tauri-apps/api/webviewWindow");
		if (getCurrentWebviewWindow().label !== "main") return;

		try {
			const pending = await takePendingOpenFile();
			if (!cancelled && pending) {
				void openProjectFromExternalPath(pending);
			}
		} catch (e) {
			console.warn("[open-recast] cold-start drain failed", e);
		}

		// Jump list "New Recording" cold start: open the panel once ready.
		try {
			if (!cancelled && (await takePendingNewRecording())) {
				void launchRecordingPanel();
			}
		} catch (e) {
			console.warn("[new-recording] cold-start drain failed", e);
		}

		const unlistenPromise = listen<string>("app://open-recast", ({ payload }) => {
			if (!payload) return;
			void openProjectFromExternalPath(payload);
		});
		unlistenPromise.then((fn) => {
			if (cancelled) fn();
			else unlistenFn = fn;
		});

		// Alt+Shift+R while idle asks the main window for the panel; stop and pause route to the panel in Rust.
		const unlistenLaunchPanel = listen("global-shortcut:launch-panel", () => {
			void launchRecordingPanel();
		});
		unlistenLaunchPanel.then((fn) => {
			if (cancelled) fn();
			else unlistenPanelFn = fn;
		});

		// Cold start: getCurrent() returns the launch URL. Warm start: onOpenUrl fires. Both route through handleDeepLink.
		try {
			const { getCurrent, onOpenUrl } = await import("@tauri-apps/plugin-deep-link");
			const startUrls = await getCurrent();
			if (!cancelled && startUrls) {
				for (const u of startUrls) void handleDeepLink(u);
			}
			const fn = await onOpenUrl((urls) => {
				for (const u of urls) void handleDeepLink(u);
			});
			if (cancelled) fn();
			else unlistenDeepLink = fn;
		} catch (e) {
			console.warn("[deep-link] setup failed", e);
		}
	};

	void setup();

	return () => {
		cancelled = true;
		unlistenFn?.();
		unlistenDeepLink?.();
		unlistenPanelFn?.();
	};
});

// Translucent backdrop for the app windows; overlays opt out, and it re-applies live when the setting is toggled.
onMount(() => {
	if (isTransparentRoute) return;
	void applyWindowBackdrop();
	const un = listen(BACKDROP_CHANGED_EVENT, () => void applyWindowBackdrop());
	return () => {
		void un.then((fn) => fn());
	};
});

// View Transitions for page changes, skipped for overlay windows and reduced motion (CSS covers that too).
onNavigate((navigation) => {
	if (typeof document === "undefined") return;
	if (!("startViewTransition" in document)) return;

	const to = navigation.to?.url.pathname ?? "";
	const from = navigation.from?.url.pathname ?? "";
	if (isOverlayRoute(to) || isOverlayRoute(from)) return;

	document.documentElement.dataset.navDirection = to.length >= from.length ? "forward" : "back";

	return new Promise((resolve) => {
		document.startViewTransition(async () => {
			resolve();
			await navigation.complete;
		});
	});
});

// Download external assets (wallpapers etc.) on first paint; no-op in browser.
initAssets();
initExtensions();

onMount(async () => {
	await tick();
	const boot = document.getElementById("boot");
	if (boot) {
		boot.classList.add("boot-leaving");
		setTimeout(() => boot.remove(), 280);
	}

	if (await isTauriApp()) {
		const theme = await getTauriTheme();
		// Defer to the OS theme when the user hasn't picked; read-only, since mode-watcher owns this key.
		const stored = safeStorage.get<string>("mode-watcher-mode", "");
		if (theme && (!stored || stored === "system")) {
			setMode(theme);
		}
	}
});

// Traces phantom-shortcut reports: a bare-modifier key firing an action means a stale HMR listener, and a doubled log means leaking listeners.
function logKeyDiagnostic(e: KeyboardEvent) {
	if (!e.ctrlKey && !e.metaKey && !e.altKey && e.key.length === 1) return;
	const t = e.target as HTMLElement | null;
	log.debug("input", "keydown", {
		key: e.key,
		code: e.code,
		ctrl: e.ctrlKey,
		meta: e.metaKey,
		shift: e.shiftKey,
		alt: e.altKey,
		repeat: e.repeat,
		target: t?.tagName?.toLowerCase() ?? null,
		route: page.url.pathname,
	});
}

// Swallowed in the CAPTURE phase so stale HMR ghosts can't act on them; a real combo carries a non-modifier key.
const BARE_MODIFIER_KEYS = new Set(["Control", "Shift", "Alt", "Meta", "OS", "AltGraph"]);
$effect(() => {
	const swallowBareModifier = (e: KeyboardEvent) => {
		if (BARE_MODIFIER_KEYS.has(e.key)) e.stopImmediatePropagation();
	};
	window.addEventListener("keydown", swallowBareModifier, { capture: true });
	return () =>
		window.removeEventListener("keydown", swallowBareModifier, {
			capture: true,
		});
});
</script>

<!-- Svelte allows one window keydown hook: diagnostics, then the dispatcher
     (skipped on overlay windows, which own their key handling). -->
<svelte:window
  onkeydown={(e) => {
    logKeyDiagnostic(e);
    if (!isTransparentRoute) dispatchShortcut(e);
  }}
/>

<TooltipProvider>
  <NavProgress active={navigating.to !== null} />
  <ModeWatcher />
  <!-- Gate the Toaster out of overlay windows (too small to host a Sonner card);
       toast.* becomes a no-op there. -->
  {#if !isTransparentRoute}
    <Toaster />
    <!-- Owns the ⌘K shortcut + dialog so they work on every route, not just (app). -->
    <CommandPaletteHost />
    <ShortcutsDialog />
  {/if}
  <div
    class="relative flex min-h-screen min-w-dvw w-full flex-col {isTransparentRoute
      ? 'bg-transparent'
      : 'bg-background'}"
  >
    {@render children()}
  </div>
  {#if showFirstRun}
    <FirstRunConsent onclose={() => (showFirstRun = false)} />
  {/if}
</TooltipProvider>
