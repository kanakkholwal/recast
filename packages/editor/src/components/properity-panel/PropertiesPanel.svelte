<script lang="ts">
import type { IconComponent } from "@recast/icons";
import {
	Blocks,
	Captions,
	DeviceComputerCamera,
	ImageIcon,
	Info,
	Lock,
	MousePointer,
	Pencil,
	ScanText,
	SquareSplitHorizontal,
	Volume,
	ZoomIn,
} from "@recast/icons";
import * as Tabs from "@recast/ui/tabs";
import { untrack } from "svelte";
import type { CameraCapture } from "../../lib/wire-types";
import type { EditorStore, PanelTab } from "../../stores/editor-store.svelte";
import AnnotationsPanel from "./AnnotationsPanel.svelte";
import AudioPanel from "./AudioPanel.svelte";
import CameraPanel from "./CameraPanel.svelte";
import CanvasPanel from "./CanvasPanel.svelte";
import CaptionsPanel from "./CaptionsPanel.svelte";
import ClipPanel from "./ClipPanel.svelte";
import CursorPanel from "./CursorPanel.svelte";
import DevOcrPanel from "./DevOcrPanel.svelte";
import ExtensionsPanel from "./ExtensionsPanel.svelte";
import FocusPanel from "./FocusPanel.svelte";
import InfoPanel from "./InfoPanel.svelte";
import MusicPanel from "./MusicPanel.svelte";

interface Props {
	store: EditorStore;
	/** Path to camera.mp4, or null when no camera was recorded. */
	cameraPath?: string | null;
	/** Why that path is or isn't set, so the panel can say which. */
	cameraCapture?: CameraCapture;
	/** Forwarded to FocusPanel; the editor page owns the auto-zoom run. */
	onRegenerateAutoZoom?: () => void;
	/** Sections this host can serve. Omit ⇒ all of them. A section the host
	 *  can't back is hidden rather than shown broken. */
	panels?: readonly PanelTab[];
	/** Someone else holds the write lock (agent session): look, don't touch. */
	readOnly?: boolean;
}

type TabGroup = "composition" | "selection" | "meta";
type TabType = {
	id: PanelTab;
	label: string;
	icon: IconComponent;
	group: TabGroup;
	// One line under the header, so an unlabeled icon rail still says where you are.
	hint: string;
};

// Clip, Zoom and Markup are selection-driven, so every id here needs a rail button or the panel strands.
const TABS: TabType[] = [
	{
		id: "canvas",
		label: "Canvas",
		icon: ImageIcon,
		group: "composition",
		hint: "Wallpaper, padding, and shadow.",
	},
	{
		id: "cursor",
		label: "Cursor",
		icon: MousePointer,
		group: "composition",
		hint: "Size, smoothing, and click effects.",
	},

	{
		id: "camera" as PanelTab,
		label: "Camera",
		icon: DeviceComputerCamera,
		group: "composition" as TabGroup,
		hint: "Webcam overlay position and shape.",
	},

	{
		id: "audio",
		label: "Audio",
		icon: Volume,
		group: "composition",
		hint: "Volume and mute.",
	},
	// TODO: re-add a music tab; the audio panel covers it for now.
	{
		id: "captions",
		label: "Captions",
		icon: Captions,
		group: "composition",
		hint: "Transcribe and style subtitles.",
	},
	{
		id: "clip",
		label: "Clip",
		icon: SquareSplitHorizontal,
		group: "selection",
		hint: "Speed of the selected clip.",
	},
	{
		id: "focus",
		label: "Zoom",
		icon: ZoomIn,
		group: "selection",
		hint: "Punch-in regions that highlight the action.",
	},
	{
		id: "annotations",
		label: "Markup",
		icon: Pencil,
		group: "selection",
		hint: "Arrows, boxes, text, and blur.",
	},
	{
		id: "extensions",
		label: "Plugins",
		icon: Blocks,
		group: "meta",
		hint: "Installed asset packs.",
	},
	{
		id: "info",
		label: "Info",
		icon: Info,
		group: "meta",
		hint: "Recording details.",
	},
	// Dev builds only; tree-shaken out of production by import.meta.env.DEV.
	...(import.meta.env.DEV
		? [
				{
					id: "dev" as PanelTab,
					label: "Screen text",
					icon: ScanText,
					group: "meta" as TabGroup,
					hint: "On-device screen text (dev).",
				},
			]
		: []),
];

const GROUP_ORDER: TabGroup[] = ["composition", "selection", "meta"];

let {
	store,
	cameraPath = null,
	cameraCapture = "legacy",
	onRegenerateAutoZoom,
	panels,
	readOnly = false,
}: Props = $props();

const visibleTabs = $derived(panels ? TABS.filter((t) => panels.includes(t.id)) : TABS);
// Grouped + ordered for the rail; empty groups drop out so dividers stay honest.
const groupedTabs = $derived(
	GROUP_ORDER.map((g) => visibleTabs.filter((t) => t.group === g)).filter((g) => g.length > 0),
);

// A host that drops the active section would leave the rail with nothing selected and the body blank.
$effect(() => {
	const tabs = visibleTabs;
	untrack(() => {
		if (tabs.length > 0 && !tabs.some((t) => t.id === store.activePanel)) {
			store.activePanel = tabs[0].id;
		}
	});
});

// Switch to Clip when a clip/segment is selected from the timeline.
$effect(() => {
	if (store.selectedClipStart !== null) {
		store.activePanel = "clip";
	}
});

// Switch to Focus when a zoom region is selected from the timeline.
$effect(() => {
	if (store.selectedZoomRegionId) {
		store.activePanel = "focus";
	}
});

// Switch to Annotations when one is selected or a tool is active.
$effect(() => {
	if (store.selectedAnnotationId || store.annotationTool) {
		store.activePanel = "annotations";
	}
});

const activeTab = $derived(visibleTabs.find((t) => t.id === store.activePanel) ?? visibleTabs[0]);

const tabContentClass = "min-h-0 flex-1 overflow-y-auto px-3 py-3 scrollbar-transparent";
</script>


{#snippet railTab(tab: TabType)}
  {@const Icon = tab.icon}
  <Tabs.Trigger
    value={tab.id}
    class="flex w-full flex-none flex-col items-center gap-1 rounded-lg px-1 py-1.5 text-[9px] font-medium leading-none transition-colors group-data-[orientation=vertical]/tabs:justify-center"
  >
    <Icon class="size-4" />
    <span class="w-full truncate text-center">{tab.label}</span>
  </Tabs.Trigger>
{/snippet}

<aside
  class="@container/panel flex h-full min-h-0 flex-row bg-background text-[12px]"
>
  <Tabs.Root
    value={store.activePanel}
    onValueChange={(v: string) => {
      store.activePanel = v as PanelTab;
    }}
    orientation="vertical"
    class="h-full shrink-0"
  >
    <Tabs.List
      variant="soft"
      class="h-full w-16 shrink-0 flex-col gap-1 overflow-y-auto border-r border-border/60 bg-transparent px-2 py-2 scrollbar-transparent no-scrollbar [&_[data-slot=tabs-trigger][data-state=active]_svg]:text-foreground"
    >
      {#each groupedTabs as group, gi (gi)}
        {#each group as tab (tab.id)}
          {@render railTab(tab)}
        {/each}
        {#if gi < groupedTabs.length - 1}
          <div
            role="separator"
            class="mx-auto my-0.5 h-px w-5 shrink-0 bg-border/60"
          ></div>
        {/if}
      {/each}
    </Tabs.List>
  </Tabs.Root>

  <div class="flex min-w-0 flex-1 flex-col">
    <header
      class="flex h-9 shrink-0 items-center gap-2 border-b border-border/60 px-3"
    >
      <h2
        class="flex shrink-0 items-center gap-1.5 text-[12px] font-semibold leading-none text-foreground"
      >
        {activeTab.label}
        {#if readOnly}
          <Lock class="size-3 text-muted-foreground" aria-hidden="true" />
        {/if}
      </h2>
      <p class="min-w-0 flex-1 truncate text-right text-[10.5px] leading-none text-muted-foreground/80">
        {readOnly ? "Read-only while the agent is editing." : activeTab.hint}
      </p>
    </header>

    <!-- `inert` covers the controls only: the rail, the header and Info stay
         live so a locked panel is still readable and navigable. -->
    <div
      class={tabContentClass}
      role="tabpanel"
      aria-label={activeTab.label}
      inert={readOnly && activeTab.id !== "info"}
    >
      {#if store.activePanel === "clip"}
        <ClipPanel {store} />
      {:else if store.activePanel === "canvas"}
        <CanvasPanel {store} />
      {:else if store.activePanel === "focus"}
        <FocusPanel {store} {onRegenerateAutoZoom} />
      {:else if store.activePanel === "annotations"}
        <AnnotationsPanel {store} />
      {:else if store.activePanel === "cursor"}
        <CursorPanel {store} />
      {:else if store.activePanel === "camera"}
        <CameraPanel {store} {cameraPath} {cameraCapture} />
      {:else if store.activePanel === "audio"}
        <AudioPanel {store} />
      {:else if store.activePanel === "music"}
        <MusicPanel {store} />
      {:else if store.activePanel === "captions"}
        <CaptionsPanel {store} />
      {:else if store.activePanel === "extensions"}
        <ExtensionsPanel {store} />
      {:else if store.activePanel === "info"}
        <InfoPanel {store} />
      {:else if store.activePanel === "dev"}
        <DevOcrPanel {store} />
      {/if}
    </div>
  </div>
</aside>
