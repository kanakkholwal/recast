<script lang="ts">
  import { CAMERA_OVERLAY_UI_ENABLED } from "$lib/feature-flags";
  import type { EditorStore, PanelTab } from "$lib/stores/editor-store.svelte";
  import type { IconComponent } from "@recast/icons";
  import {
    Blocks,
    Captions,
    ImageIcon,
    Info,
    MousePointer,
    Pencil,
    ScanText,
    SquareSplitHorizontal,
    Video,
    Volume,
    ZoomIn,
  } from "@recast/icons";
  import * as Tabs from "@recast/ui/tabs";
  import * as Tooltip from "@recast/ui/tooltip";
  import AnnotationsPanel from "./AnnotationsPanel.svelte";
  import AudioPanel from "./AudioPanel.svelte";
  import BackgroundPicker from "./BackgroundPicker.svelte";
  import CameraPanel from "./CameraPanel.svelte";
  import CaptionsPanel from "./CaptionsPanel.svelte";
  import ClipPanel from "./ClipPanel.svelte";
  import CursorPanel from "./CursorPanel.svelte";
  import DevOcrPanel from "./DevOcrPanel.svelte";
  import ExtensionsPanel from "./ExtensionsPanel.svelte";
  import FocusPanel from "./FocusPanel.svelte";
  import InfoPanel from "./InfoPanel.svelte";

  interface Props {
    store: EditorStore;
    /** Path to camera.mp4, or null when no camera was recorded. */
    cameraPath?: string | null;
  }

  // A section is one of three kinds. The rail groups by kind (thin dividers
  // between groups) so ~9 sections read as structure, not a flat wall of icons:
  //   composition  the whole video's look + sound (the panel's "home")
  //   selection    properties of whatever is selected on the timeline
  //   meta         packs + read-only info
  type TabGroup = "composition" | "selection" | "meta";
  type TabType = {
    id: PanelTab;
    label: string;
    icon: IconComponent;
    group: TabGroup;
    // One line under the header, so an unlabeled icon rail still says where you are.
    hint: string;
  };

  // Clip/Zoom/Markup are selection-driven (the effects below force the panel to
  // them when you select on the timeline), so a clip with no button would strand
  // the panel; every id here has a rail button.
  const TABS: TabType[] = [
    {
      id: "background",
      label: "Background",
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
    ...(CAMERA_OVERLAY_UI_ENABLED
      ? [
          {
            id: "camera" as PanelTab,
            label: "Camera",
            icon: Video,
            group: "composition" as TabGroup,
            hint: "Webcam overlay position and shape.",
          },
        ]
      : []),
    {
      id: "audio",
      label: "Audio",
      icon: Volume,
      group: "composition",
      hint: "Volume and mute.",
    },
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
      label: "Extensions",
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
  // Grouped + ordered for the rail; empty groups drop out so dividers stay honest.
  const groupedTabs = GROUP_ORDER.map((g) =>
    TABS.filter((t) => t.group === g),
  ).filter((g) => g.length > 0);

  let { store, cameraPath = null }: Props = $props();

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

  const activeTab = $derived(
    TABS.find((t) => t.id === store.activePanel) ?? TABS[0],
  );

  const tabContentClass =
    "min-h-0 flex-1 overflow-y-auto px-3 py-3 scrollbar-transparent";
</script>

<!-- One icon-per-section rail (fixed column, never reflows) beside the content.
     The active section's name + hint live in the content header, since the rail
     icons are unlabeled (label on hover). -->
{#snippet railTab(tab: TabType)}
  {@const Icon = tab.icon}
  <Tooltip.Root>
    <Tooltip.Trigger>
      {#snippet child({ props })}
        <Tabs.Trigger
          {...props}
          value={tab.id}
          aria-label={tab.label}
          class="relative size-7 flex-none rounded-md px-0 group-data-[orientation=vertical]/tabs:justify-center data-[state=active]:text-foreground data-[state=active]:bg-foreground/10"
        >
          <Icon class="size-4" />
        </Tabs.Trigger>
      {/snippet}
    </Tooltip.Trigger>
    <Tooltip.Content side="left">{tab.label}</Tooltip.Content>
  </Tooltip.Root>
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
      class="h-full w-11 px-1.5 shrink-0 flex-col gap-1 overflow-y-auto border-r border-b border-border/60 bg-transparent scrollbar-transparent no-scrollbar"
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
    <header class="shrink-0 border-b border-border/60 px-3 py-2.5">
      <h2 class="text-[13px] font-semibold leading-none text-foreground">
        {activeTab.label}
      </h2>
      <p class="mt-1 truncate text-[11px] leading-none text-muted-foreground">
        {activeTab.hint}
      </p>
    </header>

    <div class={tabContentClass} role="tabpanel" aria-label={activeTab.label}>
      {#if store.activePanel === "clip"}
        <ClipPanel {store} />
      {:else if store.activePanel === "background"}
        <BackgroundPicker {store} />
      {:else if store.activePanel === "focus"}
        <FocusPanel {store} />
      {:else if store.activePanel === "annotations"}
        <AnnotationsPanel {store} />
      {:else if store.activePanel === "cursor"}
        <CursorPanel {store} />
      {:else if CAMERA_OVERLAY_UI_ENABLED && store.activePanel === "camera"}
        <CameraPanel {store} {cameraPath} />
      {:else if store.activePanel === "audio"}
        <AudioPanel {store} />
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
