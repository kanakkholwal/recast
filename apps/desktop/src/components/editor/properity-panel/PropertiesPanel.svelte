<script lang="ts">
  import { CAMERA_OVERLAY_UI_ENABLED } from "$lib/feature-flags";
  import type { EditorStore, PanelTab } from "$lib/stores/editor-store.svelte";
  import {
    ImageIcon,
    Info,
    MousePointer,
    Pencil,
    Target,
    Video,
    Volume2,
  } from "@lucide/svelte";
  import * as Tabs from "@recast/ui/tabs";
  import * as Tooltip from "@recast/ui/tooltip";
  import { cn } from "@recast/ui/utils";
  import AnnotationsPanel from "./AnnotationsPanel.svelte";
  import AudioPanel from "./AudioPanel.svelte";
  import BackgroundPicker from "./BackgroundPicker.svelte";
  import CameraPanel from "./CameraPanel.svelte";
  import CursorPanel from "./CursorPanel.svelte";
  import FocusPanel from "./FocusPanel.svelte";
  import InfoPanel from "./InfoPanel.svelte";

  interface Props {
    store: EditorStore;
    /** Path to camera.mp4 in this project, or null when no camera was
     *  recorded. Forwarded to CameraPanel for its empty state. */
    cameraPath?: string | null;
  }
  type TabType = {
    id: PanelTab;
    label: string;
    icon: typeof ImageIcon;
  };
  // TODO(camera-recording): re-add the Camera tab when CAMERA_OVERLAY_UI_ENABLED
  // flips back to true. The CameraPanel component itself is intact.
  // See apps/desktop/docs/camera-recording-todo.md.
  const tabs: TabType[] = [
    { id: "background", label: "Background", icon: ImageIcon },
    { id: "focus", label: "Focus", icon: Target },
    { id: "annotations", label: "Annotations", icon: Pencil },
    { id: "cursor", label: "Cursor", icon: MousePointer },
    ...(CAMERA_OVERLAY_UI_ENABLED
      ? [{ id: "camera" as PanelTab, label: "Camera", icon: Video }]
      : []),
    { id: "audio", label: "Audio", icon: Volume2 },
    { id: "info", label: "Info", icon: Info },
  ];

  let { store, cameraPath = null }: Props = $props();

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

  const activeTabLabel = $derived(
    tabs.find((t) => t.id === store.activePanel)?.label ?? "Panel",
  );

  const tabContentClass =
    "min-h-0 flex-1 overflow-y-auto px-3 py-3 scrollbar-transparent";
</script>

<aside
  class="@container/panel flex h-full min-h-0 flex-col bg-sidebar text-[12px]"
>
  <Tabs.Root
    value={store.activePanel}
    onValueChange={(v: string) => {
      store.activePanel = v as PanelTab;
    }}
    class="flex min-h-0 flex-1 flex-col"
  >
    <!-- Header: segmented icon rail + active label -->
    <div
      class="shrink-0 flex items-center justify-between gap-2 border-b border-border/60 px-2 py-1.5"
    >
      <Tabs.List
        class="flex h-auto items-center gap-0.5 rounded-lg bg-muted/60 p-0.5 ring-1 ring-inset ring-border/40"
        variant="soft"
      >
        {#each tabs as tab}
          {@const Icon = tab.icon}
          {@const active = store.activePanel === tab.id}
          <Tooltip.Root>
            <Tooltip.Trigger>
              <Tabs.Trigger
                value={tab.id}
                class={cn(
                  "cursor-pointer flex size-6 items-center justify-center rounded-md transition-all duration-150",
                  active
                    ? "bg-card text-foreground shadow-(--shadow-craft-inset) ring-1 ring-inset ring-border/40"
                    : "text-muted-foreground hover:text-foreground",
                )}
              >
                <Icon class="size-3.5" />
                <span class="sr-only">{tab.label}</span>
              </Tabs.Trigger>
            </Tooltip.Trigger>
            <Tooltip.Content>{tab.label}</Tooltip.Content>
          </Tooltip.Root>
        {/each}
      </Tabs.List>
      <span
        class="truncate text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
      >
        {activeTabLabel}
      </span>
    </div>

    <Tabs.Content value="background" class={tabContentClass}>
      <BackgroundPicker {store} />
    </Tabs.Content>

    <Tabs.Content value="focus" class={tabContentClass}>
      <FocusPanel {store} />
    </Tabs.Content>

    <Tabs.Content value="annotations" class={tabContentClass}>
      <AnnotationsPanel {store} />
    </Tabs.Content>

    <Tabs.Content value="cursor" class={tabContentClass}>
      <CursorPanel {store} />
    </Tabs.Content>

    {#if CAMERA_OVERLAY_UI_ENABLED}
      <Tabs.Content value="camera" class={tabContentClass}>
        <CameraPanel {store} {cameraPath} />
      </Tabs.Content>
    {/if}

    <Tabs.Content value="audio" class={tabContentClass}>
      <AudioPanel {store} />
    </Tabs.Content>

    <Tabs.Content value="info" class={tabContentClass}>
      <InfoPanel {store} />
    </Tabs.Content>
  </Tabs.Root>
</aside>
