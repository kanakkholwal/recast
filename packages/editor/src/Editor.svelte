<script lang="ts" module>
import type { MediaRef } from "@recast/media";
import type { Snippet } from "svelte";
import type { PanelTab } from "./lib/editor/panel-tabs";
import type { EditorServices } from "./lib/editor/services";
import type { AudioTrackSpec } from "./lib/playback/audio-engine";
import type { TileProvider } from "./lib/timeline/filmstrip-source";
import type { CameraCapture } from "./lib/wire-types";
import type { EditorStore } from "./stores/editor-store.svelte";

export interface EditorProps {
	/** The host owns the store so it can load, save and inspect the document. */
	store: EditorStore;
	services: EditorServices;
	/** A loadable URL for the source video (object URL on web, asset URL on desktop). */
	videoSrc: string;
	/** Same source as a ref, when the host can stream it off a File. */
	video?: MediaRef;
	cameraSrc?: string;
	/** Path to the camera track, for the Camera panel's own controls. */
	cameraPath?: string | null;
	/** Why that path is or isn't set, so the panel can say which. */
	cameraCapture?: CameraCapture;
	cursorPath?: string | null;
	/** Audio the preview plays. On web this is the source's own track. */
	audioTracks?: readonly AudioTrackSpec[];
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
	onexport?: () => void;
	onsave?: () => void | Promise<void>;
	isSaving?: boolean;
	/** Forwarded to FocusPanel; the host owns the auto-zoom run. */
	onRegenerateAutoZoom?: () => void;
	/** Replaces the properties rail while the host runs its export flow. */
	exportPanel?: Snippet;
	class?: string;
}
</script>

<script lang="ts">
import { cn } from "@recast/ui/utils";
import { onDestroy, untrack } from "svelte";
import { cubicOut } from "svelte/easing";
import { slide } from "svelte/transition";
import EditorToolbar from "./components/EditorToolbar.svelte";
import PropertiesPanel from "./components/properity-panel/PropertiesPanel.svelte";
import Timeline from "./components/Timeline.svelte";
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
import { AudioTimelineEngine } from "./lib/playback/audio-engine";

let {
	store,
	services,
	videoSrc,
	video,
	cameraSrc = "",
	cameraPath = null,
	cameraCapture = "legacy",
	cursorPath = null,
	audioTracks,
	panels = PANEL_TABS,
	filename = "",
	tileProvider = null,
	filmstripVersion = 0,
	activePanel = $bindable(),
	showSidebar = $bindable(),
	showTimeline = $bindable(),
	onexport,
	onsave,
	isSaving = false,
	onRegenerateAutoZoom,
	exportPanel,
	class: className,
}: EditorProps = $props();

// Installed once, at init: capabilities are a property of the host, not state.
setEditorServices(untrack(() => services));

const storage = typeof localStorage === "undefined" ? null : localStorage;

// Layout: the host may drive it (bindable, e.g. from a URL) or leave it to us,
// in which case we seed from localStorage and remember changes.
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

let videoEl = $state<HTMLVideoElement | null>(null);
let previewContainerEl = $state<HTMLElement | null>(null);
let webcodecsActive = $state(false);
let captureFrame = $state<(() => Promise<Blob | null>) | undefined>(undefined);
let loopEnabled = $state(false);

// --- panel sizing ---
// Measured so the timeline's ceiling is a share of the space actually
// available, not a fixed number that overwhelms a short window.
let editorColumnH = $state(0);
let sidebarWidth = $state(
	clampSidebarWidth(readStoredNumber(storage?.getItem(SIDEBAR_WIDTH_KEY) ?? null, SIDEBAR_DEFAULT_WIDTH_PX)),
);
let timelineHeight = $state(
	readStoredNumber(storage?.getItem(TIMELINE_HEIGHT_KEY) ?? null, TIMELINE_DEFAULT_HEIGHT_PX),
);
let resizingSidebar = $state(false);
let resizingTimeline = $state(false);

const timelineMax = $derived(timelineMaxHeight(editorColumnH));

// Re-clamp when the window changes, so a panel sized in a big window doesn't
// swallow a small one.
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

// --- audio ---
// Rebuilt whenever the track list changes. Adopting a stale engine would strand
// its AudioContext (an OS audio thread) plus its decoded PCM.
let audioEngine = $state<AudioTimelineEngine | null>(null);
let audioGen = 0;

$effect(() => {
	const specs = audioTracks;
	const gen = ++audioGen;
	const previous = untrack(() => audioEngine);
	previous?.dispose();
	audioEngine = null;
	if (!specs?.length) return;
	void AudioTimelineEngine.create([...specs])
		.then((engine) => {
			if (gen !== audioGen) {
				engine.dispose();
				return;
			}
			audioEngine = engine;
		})
		.catch(() => {
			// Nothing decodable: the preview stays silent rather than failing.
		});
});

onDestroy(() => {
	audioGen++;
	audioEngine?.dispose();
});

function handleTimeUpdate() {
	// The WebCodecs clock owns `store.currentTime`; echoing the element's time
	// while it free-runs through the un-cut source snaps playback across cuts.
	if (webcodecsActive || !videoEl) return;
	store.currentTime = videoEl.currentTime;
}

function handleEnded(): boolean {
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
		"bg-background text-foreground flex h-full min-h-0 w-full flex-col overflow-hidden",
		className,
	)}
>
	<!-- The toolbar's own root is `h-full`, so it needs a sized, non-shrinking
	     row here or it expands to the whole column (desktop sizes it via
	     CustomTitlebar's h-9 wrapper). -->
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

	<div class="flex min-h-0 flex-1 overflow-hidden">
		<!-- Preview + playback + timeline -->
		<div bind:clientHeight={editorColumnH} class="flex min-h-0 flex-1 flex-col overflow-hidden">
			<div
				bind:this={previewContainerEl}
				class="bg-background flex min-h-0 flex-1 flex-col items-center justify-center px-2 pt-1.5 pb-1"
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
						onTimeUpdate={handleTimeUpdate}
						onEnded={handleEnded}
						onLoadedMetadata={() => {}}
						onReady={() => {}}
						onError={() => {}}
						audioPositionSec={() => audioEngine?.positionOutputSec ?? null}
					/>
				</div>
				<VideoPlayerControls
					{store}
					{videoEl}
					{captureFrame}
					bind:loopEnabled
					fullscreenTargetEl={previewContainerEl}
					showScrubber={!timelineOpen}
				/>
			</div>

			<!-- `slide` (axis:y) animates the wrapper height to 0 while the inner keeps
			     its height, so the preview reclaims the space smoothly. -->
			{#if timelineOpen}
				<div
					class="shrink-0 overflow-hidden"
					transition:slide={{ axis: "y", duration: 280, easing: cubicOut }}
				>
					<!-- Height on the INNER div: `slide` animates the wrapper's own
					     height, so the two would otherwise fight over one property. -->
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
						<Timeline {store} {videoEl} {tileProvider} {filmstripVersion} />
					</div>
				</div>
			{/if}
		</div>

		<!-- Right rail. Editing shows the properties panel; a host running an
		     export flow swaps it for that surface. Both slide on the x-axis with the
		     same duration/easing so the widths cancel to a monotonic reflow. -->
		{#if exportPanel}
			<aside
				class="border-border/60 min-h-0 shrink-0 overflow-hidden border-l"
				transition:slide={{ axis: "x", duration: 280, easing: cubicOut }}
			>
				<div class="h-full w-[26rem]">{@render exportPanel()}</div>
			</aside>
		{:else if sidebarOpen}
			<aside
				class="border-border/60 relative min-h-0 shrink-0 overflow-hidden border-l"
				transition:slide={{ axis: "x", duration: 280, easing: cubicOut }}
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
					<PropertiesPanel {store} {cameraPath} {cameraCapture} {onRegenerateAutoZoom} {panels} />
				</div>
			</aside>
		{/if}
	</div>
</div>
