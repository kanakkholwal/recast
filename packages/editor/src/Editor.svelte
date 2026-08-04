<script lang="ts" module>
import type { MediaRef } from "@recast/media";
import type { Snippet } from "svelte";
import type { PanelTab } from "./lib/editor/panel-tabs";
import type { EditorServices } from "./lib/editor/services";
import type { AudioTrackSpec } from "./lib/playback/audio-engine";
import type { TileProvider } from "./lib/timeline/filmstrip-source";
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
	/** Host chrome for the toolbar's trailing edge (share, upload, a CTA). */
	toolbarEnd?: Snippet;
	class?: string;
}
</script>

<script lang="ts">
import { cn } from "@recast/ui/utils";
import { onDestroy, untrack } from "svelte";
import EditorToolbar from "./components/EditorToolbar.svelte";
import PropertiesPanel from "./components/properity-panel/PropertiesPanel.svelte";
import Timeline from "./components/Timeline.svelte";
import VideoPlayerControls from "./components/VideoPlayerControls.svelte";
import VideoPreview from "./components/VideoPreview.svelte";
import {
	clampSidebarWidth,
	endOfClipAction,
	LAYOUT_KEY,
	parseLayout,
	SIDEBAR_DEFAULT,
	SIDEBAR_MAX,
	SIDEBAR_MIN,
	SIDEBAR_WIDTH_KEY,
} from "./editor-shell.logic";
import { PANEL_TABS } from "./lib/editor/panel-tabs";
import { setEditorServices } from "./lib/editor/services";
import { AudioTimelineEngine } from "./lib/playback/audio-engine";

let {
	store,
	services,
	videoSrc,
	cameraSrc = "",
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
	toolbarEnd,
	class: className,
}: EditorProps = $props();

// Installed once, at init: capabilities are a property of the host, not state.
setEditorServices(untrack(() => services));

// Layout: the host may drive it (bindable, e.g. from a URL) or leave it to us,
// in which case we seed from localStorage and remember changes.
const seeded = parseLayout(
	typeof localStorage === "undefined" ? null : localStorage.getItem(LAYOUT_KEY),
);
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
		localStorage.setItem(LAYOUT_KEY, JSON.stringify(next));
	} catch {
		// Private mode / quota: the toggle still works, it just isn't remembered.
	}
});

let sidebarWidth = $state(
	clampSidebarWidth(
		typeof localStorage === "undefined"
			? SIDEBAR_DEFAULT
			: Number(localStorage.getItem(SIDEBAR_WIDTH_KEY)),
	),
);
$effect(() => {
	const w = sidebarWidth;
	try {
		localStorage.setItem(SIDEBAR_WIDTH_KEY, String(w));
	} catch {
		// see above
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
let previewEl = $state<HTMLElement | null>(null);
let webcodecsActive = $state(false);
let captureFrame = $state<(() => Promise<Blob | null>) | undefined>(undefined);
let loopEnabled = $state(false);

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

const audioPositionSec = () => audioEngine?.positionOutputSec ?? null;

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

function startSidebarResize(event: PointerEvent) {
	const startX = event.clientX;
	const startWidth = sidebarWidth;
	const move = (e: PointerEvent) => {
		sidebarWidth = clampSidebarWidth(startWidth + (startX - e.clientX));
	};
	const up = () => {
		window.removeEventListener("pointermove", move);
		window.removeEventListener("pointerup", up);
	};
	window.addEventListener("pointermove", move);
	window.addEventListener("pointerup", up);
}

function nudgeSidebar(delta: number) {
	sidebarWidth = clampSidebarWidth(sidebarWidth + delta);
}
</script>

<div class={cn("bg-background flex h-full min-h-0 w-full flex-col", className)}>
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
	{#if toolbarEnd}
		<div class="border-border/50 flex items-center justify-end gap-2 border-b px-3 py-1.5">
			{@render toolbarEnd()}
		</div>
	{/if}

	<div class="flex min-h-0 flex-1">
		<div class="flex min-w-0 flex-1 flex-col">
			<div bind:this={previewEl} class="relative flex min-h-0 flex-1 items-center justify-center">
				<VideoPreview
					{store}
					{videoEl}
					{videoSrc}
					{cursorPath}
					{cameraSrc}
					bind:webcodecsActive
					bind:captureFrame
					{audioPositionSec}
					onTimeUpdate={handleTimeUpdate}
					onEnded={handleEnded}
					onLoadedMetadata={() => {}}
					onReady={() => {}}
					onError={() => {}}
				/>
			</div>

			<VideoPlayerControls
				{store}
				{videoEl}
				fullscreenTargetEl={previewEl}
				{captureFrame}
				bind:loopEnabled
				showScrubber={!timelineOpen}
			/>

			{#if timelineOpen}
				<Timeline {store} {videoEl} {tileProvider} {filmstripVersion} />
			{/if}
		</div>

		{#if sidebarOpen}
			<!-- Keyboard-resizable so the panel width isn't pointer-only. A focusable
			     separator is the ARIA window-splitter pattern, which the a11y rule
			     below doesn't model. -->
			<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
			<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
			<div
				class="hover:bg-primary/40 focus-visible:bg-primary/60 w-1 shrink-0 cursor-col-resize"
				role="separator"
				aria-orientation="vertical"
				aria-label="Resize properties panel"
				aria-valuenow={sidebarWidth}
				aria-valuemin={SIDEBAR_MIN}
				aria-valuemax={SIDEBAR_MAX}
				tabindex="0"
				onpointerdown={startSidebarResize}
				onkeydown={(e) => {
					if (e.key === "ArrowLeft") nudgeSidebar(16);
					else if (e.key === "ArrowRight") nudgeSidebar(-16);
					else return;
					e.preventDefault();
				}}
			></div>
			<aside
				class="border-border/50 min-h-0 shrink-0 overflow-y-auto border-l"
				style="width: {sidebarWidth}px"
			>
				<PropertiesPanel {store} />
			</aside>
		{/if}
	</div>
</div>
