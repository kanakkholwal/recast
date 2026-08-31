<script lang="ts" module>
import type { MediaRef } from "@recast/media";
import type { Snippet } from "svelte";
import type { PanelTab } from "./lib/editor/panel-tabs";
import type { EditorServices } from "./lib/editor/services";
import type { AudioTimelineEngine } from "./lib/playback/audio-engine";
import type { TileProvider } from "./lib/timeline/filmstrip-source";
import type { CameraCapture } from "./lib/wire-types";
import type { EditorStore } from "./stores/editor-store.svelte";

/** Handed to the `toolbar` snippet so a host-owned toolbar drives the shell. */
export interface ToolbarControls {
	showSidebar: boolean;
	showTimeline: boolean;
	toggleSidebar: () => void;
	toggleTimeline: () => void;
}

export interface EditorProps {
	/** The host owns the store so it can load, save and inspect the document. */
	store: EditorStore;
	services: EditorServices;
	/** A loadable URL for the source video (object URL on web, asset URL on desktop). */
	videoSrc: string;
	/** Same source as a ref, when the host can stream it off a File. */
	video?: MediaRef;
	cameraSrc?: string;
	/** Milliseconds the camera track lags video frame 0 (measured at capture). */
	cameraOffsetMs?: number;
	/** Path to the camera track, for the Camera panel's own controls. */
	cameraPath?: string | null;
	/** Why that path is or isn't set, so the panel can say which. */
	cameraCapture?: CameraCapture;
	cursorPath?: string | null;
	/** Transport audio. HOST-owned: an AudioContext is an OS audio thread, and a
	 *  host driving its own transport must not race a second engine. Build it
	 *  with `createAudioEngineHost`. */
	audioEngine?: AudioTimelineEngine | null;
	/** Which properties tabs this host serves. Defaults to all of them. */
	panels?: readonly PanelTab[];
	filename?: string;
	/** Filmstrip tiles. The host builds it so it can pick the source strategy. */
	tileProvider?: TileProvider | null;
	filmstripVersion?: number;
	/** View state, bindable so a host can mirror it into its own URL. */
	activePanel?: PanelTab;
	showSidebar?: boolean;
	showTimeline?: boolean;
	/** Preview internals, bindable for a host that owns the transport (desktop
	 *  drives its own loop, drift correction and `<audio>` fallback off these). */
	videoEl?: HTMLVideoElement | null;
	/** The preview box, for a host that owns its own fullscreen shortcut. */
	previewContainerEl?: HTMLElement | null;
	captureFrame?: () => Promise<Blob | null>;
	webcodecsActive?: boolean;
	loopEnabled?: boolean;
	/** Transport hooks. Omitted ⇒ the built-in defaults below, which are what a
	 *  host with no audio of its own (the web playground) wants. A host that
	 *  supplies one owns that behaviour entirely. */
	onTimeUpdate?: () => void;
	/** Return true to keep the clock running (the host looped). */
	onEnded?: () => boolean;
	onLoadedMetadata?: () => void;
	onReady?: () => void;
	onError?: () => void;
	onSeeked?: () => void;
	/** Audio clock position for A/V drift correction. Defaults to `audioEngine`. */
	audioPositionSec?: () => number | null;
	/** Block structural edits in the timeline / properties panel (agent write
	 *  lock). The timeline's transport stays live either way. */
	timelineReadOnly?: boolean;
	panelReadOnly?: boolean;
	onexport?: () => void;
	onsave?: () => void | Promise<void>;
	isSaving?: boolean;
	/** Forwarded to FocusPanel; the host owns the auto-zoom run. */
	onRegenerateAutoZoom?: () => void;
	/** Replaces the properties rail while the host runs its export flow. */
	exportPanel?: Snippet;
	/** Replaces the default toolbar row, for hosts that own window chrome
	 *  (a native titlebar, status badges). Receives the panel toggles so the
	 *  host's own toolbar still drives the shell. */
	toolbar?: Snippet<[ToolbarControls]>;
	/** Full-width strip directly under the toolbar, for inline notices. */
	banner?: Snippet;
	/** Rendered at the shell root: dialogs, portals, activity hosts. */
	overlays?: Snippet;
	class?: string;
}
</script>

<script lang="ts">
import { cn } from "@recast/ui/utils";
import { untrack } from "svelte";
import { cubicOut } from "svelte/easing";
import { slide } from "svelte/transition";
import { motionDuration } from "./lib/motion.svelte";
import EditorToolbar from "./components/EditorToolbar.svelte";
import PropertiesPanel from "./components/properity-panel/PropertiesPanel.svelte";
import Timeline from "./components/Timeline.svelte";
import AspectPicker from "./components/_components/AspectPicker.svelte";
import MarkupControls from "./components/_components/MarkupControls.svelte";
import StageViewControls from "./components/_components/StageViewControls.svelte";
import TimelineCanvas from "./components/_components/timeline/TimelineCanvas.svelte";
import VideoPlayerControls from "./components/VideoPlayerControls.svelte";
import VideoPreview from "./components/VideoPreview.svelte";
import {
	endOfClipAction,
	LAYOUT_KEY,
	parseLayout,
	readStoredNumber,
	SIDEBAR_WIDTH_KEY,
	TIMELINE_HEIGHT_KEY,
} from "./editor-shell.logic";
import { PANEL_TABS } from "./lib/editor/panel-tabs";
import {
	clampSidebarWidth,
	clampTimelineHeight,
	SIDEBAR_DEFAULT_WIDTH_PX,
	SIDEBAR_MAX_WIDTH_PX,
	SIDEBAR_MIN_WIDTH_PX,
	TIMELINE_DEFAULT_HEIGHT_PX,
	TIMELINE_MIN_HEIGHT_PX,
	timelineMaxHeight,
} from "./lib/editor/panel-size";
import { setEditorServices } from "./lib/editor/services";

let {
	store,
	services,
	videoSrc,
	video,
	cameraSrc = "",
	cameraOffsetMs = 0,
	cameraPath = null,
	cameraCapture = "legacy",
	cursorPath = null,
	audioEngine = null,
	panels = PANEL_TABS,
	filename = "",
	tileProvider = null,
	filmstripVersion = 0,
	activePanel = $bindable(),
	showSidebar = $bindable(),
	showTimeline = $bindable(),
	videoEl = $bindable(null),
	previewContainerEl = $bindable(null),
	captureFrame = $bindable(),
	webcodecsActive = $bindable(false),
	loopEnabled = $bindable(false),
	onTimeUpdate,
	onEnded,
	onLoadedMetadata,
	onReady,
	onError,
	onSeeked,
	audioPositionSec,
	timelineReadOnly = false,
	panelReadOnly = false,
	onexport,
	onsave,
	isSaving = false,
	onRegenerateAutoZoom,
	exportPanel,
	toolbar,
	banner,
	overlays,
	class: className,
}: EditorProps = $props();

// Installed once, at init: capabilities are a property of the host, not state.
setEditorServices(untrack(() => services));

const storage = typeof localStorage === "undefined" ? null : localStorage;

// The host may drive layout (bindable, e.g. from a URL) or leave it to us, in which case localStorage seeds it.
const seeded = parseLayout(storage?.getItem(LAYOUT_KEY) ?? null);
let sidebarOpen = $state(showSidebar ?? seeded.sidebar);
let timelineOpen = $state(showTimeline ?? seeded.timeline);

$effect(() => {
	const s = showSidebar;
	const t = showTimeline;
	untrack(() => {
		if (s !== undefined && s !== sidebarOpen) sidebarOpen = s;
		if (t !== undefined && t !== timelineOpen) timelineOpen = t;
	});
});

$effect(() => {
	const next = { sidebar: sidebarOpen, timeline: timelineOpen };
	untrack(() => {
		if (showSidebar !== undefined) showSidebar = next.sidebar;
		if (showTimeline !== undefined) showTimeline = next.timeline;
	});
	try {
		storage?.setItem(LAYOUT_KEY, JSON.stringify(next));
	} catch {
		// Private mode / quota: the toggle still works, it just isn't remembered.
	}
});

$effect(() => {
	const tab = activePanel;
	untrack(() => {
		if (tab && tab !== store.activePanel && panels.includes(tab)) store.activePanel = tab;
	});
});
$effect(() => {
	const tab = store.activePanel;
	untrack(() => {
		if (activePanel !== undefined && activePanel !== tab) activePanel = tab;
	});
});


// Fullscreen shows only the picture + video controls, so the stage drawers hide.
let isFullscreen = $state(false);
$effect(() => {
	const onChange = () => (isFullscreen = Boolean(document.fullscreenElement));
	document.addEventListener("fullscreenchange", onChange);
	return () => document.removeEventListener("fullscreenchange", onChange);
});

// Opening export drops annotation selection/tool so the preview shows the clean composite.
$effect(() => {
	if (exportPanel) {
		untrack(() => {
			store.selectedAnnotationId = null;
			store.annotationTool = null;
		});
	}
});

// --- Panel sizing, measured so the timeline's ceiling is a share of the available space, not a fixed number.
let editorColumnH = $state(0);
let sidebarWidth = $state(
	clampSidebarWidth(readStoredNumber(storage?.getItem(SIDEBAR_WIDTH_KEY) ?? null, SIDEBAR_DEFAULT_WIDTH_PX)),
);
let timelineHeight = $state(
	readStoredNumber(storage?.getItem(TIMELINE_HEIGHT_KEY) ?? null, TIMELINE_DEFAULT_HEIGHT_PX),
);
let resizingSidebar = $state(false);
let resizingTimeline = $state(false);

// Dev flag for the Stage-A canvas timeline (chip toggle, persisted) to compare live against the DOM one.
const TL_CANVAS_KEY = "recast:tl-canvas";
// Default ON for testing the canvas timeline; the chip flips back to the DOM one.
let timelineCanvasFlag = $state(storage?.getItem(TL_CANVAS_KEY) !== "0");
function toggleTimelineCanvas() {
	timelineCanvasFlag = !timelineCanvasFlag;
	storage?.setItem(TL_CANVAS_KEY, timelineCanvasFlag ? "1" : "0");
}

const timelineMax = $derived(timelineMaxHeight(editorColumnH));

// Re-clamp on window changes, so a panel sized in a big window doesn't swallow a small one.
$effect(() => {
	const column = editorColumnH;
	untrack(() => {
		const clamped = clampTimelineHeight(timelineHeight, column);
		if (clamped !== timelineHeight) timelineHeight = clamped;
	});
});

$effect(() => {
	const w = sidebarWidth;
	const h = timelineHeight;
	try {
		storage?.setItem(SIDEBAR_WIDTH_KEY, String(w));
		storage?.setItem(TIMELINE_HEIGHT_KEY, String(h));
	} catch {
		// see above
	}
});

function handleTimeUpdate() {
	if (onTimeUpdate) return onTimeUpdate();
	// The WebCodecs clock owns `store.currentTime`; echoing the free-running element snaps playback across cuts.
	if (webcodecsActive || !videoEl) return;
	store.currentTime = videoEl.currentTime;
}

function handleEnded(): boolean {
	if (onEnded) return onEnded();
	if (endOfClipAction(loopEnabled) === "pause") {
		store.isPlaying = false;
		return false;
	}
	store.seek(store.trimStart);
	return true;
}

// --- splitters ---
function startSidebarResize(event: PointerEvent) {
	event.preventDefault();
	resizingSidebar = true;
	const startX = event.clientX;
	const startWidth = sidebarWidth;
	const move = (e: PointerEvent) => {
		sidebarWidth = clampSidebarWidth(startWidth - (e.clientX - startX));
	};
	const up = () => {
		resizingSidebar = false;
		window.removeEventListener("pointermove", move);
		window.removeEventListener("pointerup", up);
	};
	window.addEventListener("pointermove", move);
	window.addEventListener("pointerup", up);
}

function onSidebarHandleKey(event: KeyboardEvent) {
	const step = event.shiftKey ? 48 : 16;
	if (event.key === "ArrowLeft") sidebarWidth = clampSidebarWidth(sidebarWidth + step);
	else if (event.key === "ArrowRight") sidebarWidth = clampSidebarWidth(sidebarWidth - step);
	else if (event.key === "Home") sidebarWidth = SIDEBAR_MAX_WIDTH_PX;
	else if (event.key === "End") sidebarWidth = SIDEBAR_MIN_WIDTH_PX;
	else return;
	event.preventDefault();
}

function startTimelineResize(event: PointerEvent) {
	event.preventDefault();
	resizingTimeline = true;
	const startY = event.clientY;
	const startHeight = timelineHeight;
	const move = (e: PointerEvent) => {
		timelineHeight = clampTimelineHeight(startHeight - (e.clientY - startY), editorColumnH);
	};
	const up = () => {
		resizingTimeline = false;
		window.removeEventListener("pointermove", move);
		window.removeEventListener("pointerup", up);
	};
	window.addEventListener("pointermove", move);
	window.addEventListener("pointerup", up);
}

function onTimelineHandleKey(event: KeyboardEvent) {
	const step = event.shiftKey ? 48 : 16;
	if (event.key === "ArrowUp") timelineHeight = clampTimelineHeight(timelineHeight + step, editorColumnH);
	else if (event.key === "ArrowDown")
		timelineHeight = clampTimelineHeight(timelineHeight - step, editorColumnH);
	else if (event.key === "Home") timelineHeight = timelineMax;
	else if (event.key === "End") timelineHeight = TIMELINE_MIN_HEIGHT_PX;
	else return;
	event.preventDefault();
}
</script>

<div
	class={cn(
		"bg-[var(--editor-canvas)] text-foreground flex h-full min-h-0 w-full flex-col overflow-hidden",
		className,
	)}
>
	<!-- The toolbar's own root is `h-full`, so it needs a sized, non-shrinking
	     row here or it expands to the whole column (desktop sizes it via
	     CustomTitlebar's h-9 wrapper). -->
	{#if toolbar}
		{@render toolbar({
			showSidebar: sidebarOpen,
			showTimeline: timelineOpen,
			toggleSidebar: () => (sidebarOpen = !sidebarOpen),
			toggleTimeline: () => (timelineOpen = !timelineOpen),
		})}
	{:else}
		<div class="h-9 shrink-0">
			<EditorToolbar
				{store}
				{filename}
				{onexport}
				{onsave}
				{isSaving}
				showSidebar={sidebarOpen}
				showTimeline={timelineOpen}
				onToggleSidebar={() => (sidebarOpen = !sidebarOpen)}
				onToggleTimeline={() => (timelineOpen = !timelineOpen)}
			/>
		</div>
	{/if}
	{@render banner?.()}

	<div bind:clientHeight={editorColumnH} class="flex min-h-0 flex-1 flex-col overflow-hidden">
		<!-- Top row: preview + right-rail properties panel. -->
		<div class="flex min-h-0 flex-1 overflow-hidden">
			<!-- Preview + playback -->
			<div class="flex min-h-0 flex-1 flex-col overflow-hidden">
			<div
				bind:this={previewContainerEl}
				class="flex min-h-0 flex-1 flex-col items-center justify-center px-2 pt-1.5 pb-1"
			>
				<div class="relative flex min-h-0 w-full flex-1 items-center justify-center">
					<VideoPreview
						{store}
						bind:videoEl
						bind:captureFrame
						bind:webcodecsActive
						{videoSrc}
						{video}
						{cursorPath}
						{cameraSrc}
						{cameraOffsetMs}
						onTimeUpdate={handleTimeUpdate}
						onEnded={handleEnded}
						onLoadedMetadata={onLoadedMetadata ?? (() => {})}
						onReady={onReady ?? (() => {})}
						onError={onError ?? (() => {})}
						{onSeeked}
						audioPositionSec={audioPositionSec ??
							(() => audioEngine?.positionOutputSec ?? null)}
					/>
					<!-- Markup tools dock to the left edge (Markup tab only); hidden in
					     fullscreen and export, which show only picture + video controls. -->
					{#if !isFullscreen && !exportPanel}
						<div class="pointer-events-none absolute inset-y-0 left-0 z-20 flex items-center">
							<div class="pointer-events-auto">
								<MarkupControls {store} vertical />
							</div>
						</div>
					{/if}
				</div>
				<!-- Bottom control row: aspect (left), scrubber/transport (centre), view (right). -->
				<div class="flex w-full max-w-280 items-center gap-2 px-2">
					{#if !isFullscreen && !exportPanel}
						<AspectPicker {store} />
					{/if}
					<div class="min-w-0 flex-1">
						<VideoPlayerControls {store} {videoEl} showScrubber={!timelineOpen} />
					</div>
					<StageViewControls {store} {captureFrame} fullscreenTargetEl={previewContainerEl} />
				</div>
			</div>

		</div>

		<!-- Right rail. Editing shows the properties panel; a host running an
		     export flow swaps it for that surface. Both slide on the x-axis with the
		     same duration/easing so the widths cancel to a monotonic reflow. -->
		{#if exportPanel}
			<aside
				class="border-border/60 min-h-0 shrink-0 overflow-hidden border-l"
				transition:slide={{ axis: "x", duration: motionDuration(280), easing: cubicOut }}
			>
				<div class="h-full w-[26rem]">{@render exportPanel()}</div>
			</aside>
		{:else if sidebarOpen}
			<aside
				class="border-border/60 relative min-h-0 shrink-0 overflow-hidden border-l"
				transition:slide={{ axis: "x", duration: motionDuration(280), easing: cubicOut }}
			>
				<!-- Sits in the left padding gutter so it never overlaps a tab. -->
				<div
					role="slider"
					tabindex="0"
					aria-orientation="vertical"
					aria-label="Resize properties panel"
					aria-valuemin={SIDEBAR_MIN_WIDTH_PX}
					aria-valuemax={SIDEBAR_MAX_WIDTH_PX}
					aria-valuenow={sidebarWidth}
					onpointerdown={startSidebarResize}
					onkeydown={onSidebarHandleKey}
					class="group absolute inset-y-0 left-0 z-20 w-1.5 cursor-col-resize focus-visible:outline-none"
				>
					<div
						class="bg-border/50 group-hover:bg-primary/60 group-focus-visible:bg-primary mx-auto h-full w-px transition-colors {resizingSidebar
							? 'bg-primary!'
							: ''}"
					></div>
				</div>
				<div class="h-full" style="width: {sidebarWidth}px;">
					<PropertiesPanel
						{store}
						{cameraPath}
						{cameraCapture}
						{onRegenerateAutoZoom}
						{panels}
						readOnly={panelReadOnly}
					/>
				</div>
			</aside>
		{/if}
		</div>

		<!-- Timeline: FULL WIDTH along the bottom, under both preview and properties. -->
		{#if timelineOpen && !exportPanel}
			<div
				class="shrink-0 overflow-hidden"
				transition:slide={{ axis: "y", duration: motionDuration(280), easing: cubicOut }}
			>
				<div class="relative" style="height: {timelineHeight}px;">
					<div
						role="slider"
						tabindex="0"
						aria-orientation="horizontal"
						aria-label="Resize timeline"
						aria-valuemin={TIMELINE_MIN_HEIGHT_PX}
						aria-valuemax={timelineMax}
						aria-valuenow={timelineHeight}
						onpointerdown={startTimelineResize}
						onkeydown={onTimelineHandleKey}
						class="group absolute inset-x-0 top-0 z-20 h-1.5 cursor-row-resize focus-visible:outline-none"
					>
						<div
							class="bg-border/50 group-hover:bg-primary/60 group-focus-visible:bg-primary my-auto h-px w-full transition-colors {resizingTimeline
								? 'bg-primary!'
								: ''}"
						></div>
					</div>
					{#if timelineCanvasFlag}
						<TimelineCanvas {store} {videoEl} {tileProvider} {filmstripVersion} bind:loopEnabled />
					{:else}
						<Timeline
							{store}
							{videoEl}
							{tileProvider}
							{filmstripVersion}
							readOnly={timelineReadOnly}
						/>
					{/if}
					<button
						type="button"
						onclick={toggleTimelineCanvas}
						title="Toggle the Stage-A canvas timeline (dev)"
						class="absolute bottom-1.5 right-2 z-30 rounded bg-muted/70 px-1.5 py-0.5 text-[9px] font-medium text-muted-foreground transition-colors hover:text-foreground"
					>
						{timelineCanvasFlag ? "canvas ✓" : "canvas"}
					</button>
				</div>
			</div>
		{/if}
	</div>
	{@render overlays?.()}
</div>
