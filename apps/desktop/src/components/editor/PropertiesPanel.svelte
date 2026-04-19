<script lang="ts">
  import type { EditorStore, PanelTab } from "$lib/stores/editor-store.svelte";
  import { Clock, Film, Gauge, ImageIcon, MousePointer, Pencil, Target, Volume2 } from "@lucide/svelte";
  import * as Tabs from "@recast/ui/tabs";
  import * as Tooltip from "@recast/ui/tooltip";
  import AnnotationsPanel from "./AnnotationsPanel.svelte";
  import AudioPanel from "./AudioPanel.svelte";
  import BackgroundPicker from "./BackgroundPicker.svelte";
  import CursorPanel from "./CursorPanel.svelte";
  import FocusPanel from "./FocusPanel.svelte";

  interface Props {
    store: EditorStore;
  }

  const tabs: { id: PanelTab; label: string; icon: typeof ImageIcon }[] = [
    { id: "background", label: "Background", icon: ImageIcon },
    { id: "focus", label: "Focus", icon: Target },
    { id: "annotations", label: "Annotations", icon: Pencil },
    { id: "cursor", label: "Cursor", icon: MousePointer },
    { id: "audio", label: "Audio", icon: Volume2 },
  ];

  let { store }: Props = $props();

  // When a zoom region is selected from the timeline, switch to the Focus tab
  // so the user lands on the relevant editor.
  $effect(() => {
    if (store.selectedZoomRegionId) {
      store.activePanel = "focus";
    }
  });

  // Same idea for annotations — select → jump to the Annotations tab.
  $effect(() => {
    if (store.selectedAnnotationId || store.annotationTool) {
      store.activePanel = "annotations";
    }
  });

  function formatDuration(seconds: number | undefined) {
    if (!seconds || seconds <= 0) return "--:--";
    const t = Math.round(seconds);
    return `${Math.floor(t / 60)}:${(t % 60).toString().padStart(2, "0")}`;
  }

  function formatResolution() {
    if (!store.metadata?.width || !store.metadata?.height) return "Unknown";
    return `${store.metadata.width}×${store.metadata.height}`;
  }

  function formatFps() {
    if (!store.metadata?.fps) return "--";
    return `${Math.round(store.metadata.fps)} fps`;
  }

  const activeTabLabel = $derived(tabs.find((t) => t.id === store.activePanel)?.label ?? "Panel");
</script>

<aside class="@container/panel flex h-full min-h-0 flex-col bg-card text-[12px]">
  <!-- Metadata header row: source specs in Raycast-style pinned badges -->
  <header class="shrink-0 border-b border-border px-3 py-2">
    <div class="flex items-center justify-between gap-2">
      <span
        class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground"
      >
        Source
      </span>
    </div>
    <div class="mt-1.5 flex flex-wrap items-center gap-1 text-[11px]">
      <span
        class="inline-flex items-center gap-1 rounded border border-border bg-background px-1.5 py-0.5 font-mono tabular-nums text-foreground"
        title="Duration"
      >
        <Clock size={10} class="text-muted-foreground" />
        {formatDuration(store.metadata?.duration)}
      </span>
      <span
        class="inline-flex items-center gap-1 rounded border border-border bg-background px-1.5 py-0.5 font-mono tabular-nums text-foreground"
        title="Resolution"
      >
        <Film size={10} class="text-muted-foreground" />
        {formatResolution()}
      </span>
      <span
        class="inline-flex items-center gap-1 rounded border border-border bg-background px-1.5 py-0.5 font-mono tabular-nums text-foreground"
        title="Frame rate"
      >
        <Gauge size={10} class="text-muted-foreground" />
        {formatFps()}
      </span>
    </div>
  </header>

  <Tabs.Root
    value={store.activePanel}
    onValueChange={(v) => { store.activePanel = v as PanelTab; }}
    class="flex min-h-0 flex-1 flex-col"
  >
    <!-- Tabs row: dense icon buttons + current tab label -->
    <div
      class="shrink-0 flex items-center justify-between gap-2 border-b border-border px-2 py-1.5"
    >
      <Tabs.List class="h-auto gap-0.5 bg-transparent p-0">
        {#each tabs as tab}
          {@const Icon = tab.icon}
          <Tooltip.Root>
            <Tooltip.Trigger>
              <Tabs.Trigger
                value={tab.id}
                class="cursor-pointer flex size-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted/50 hover:text-foreground data-[state=active]:bg-muted data-[state=active]:text-primary"
              >
                <Icon size={13} />
                <span class="sr-only">{tab.label}</span>
              </Tabs.Trigger>
            </Tooltip.Trigger>
            <Tooltip.Content>{tab.label}</Tooltip.Content>
          </Tooltip.Root>
        {/each}
      </Tabs.List>
      <span
        class="truncate text-[10px] font-semibold uppercase tracking-wider text-muted-foreground"
      >
        {activeTabLabel}
      </span>
    </div>

    <Tabs.Content value="background" class="min-h-0 flex-1 overflow-y-auto px-3 py-3 scrollbar-transparent">
      <BackgroundPicker {store} />
    </Tabs.Content>

    <Tabs.Content value="focus" class="min-h-0 flex-1 overflow-y-auto px-3 py-3 scrollbar-transparent">
      <FocusPanel {store} />
    </Tabs.Content>

    <Tabs.Content value="annotations" class="min-h-0 flex-1 overflow-y-auto px-3 py-3 scrollbar-transparent">
      <AnnotationsPanel {store} />
    </Tabs.Content>

    <Tabs.Content value="cursor" class="min-h-0 flex-1 overflow-y-auto px-3 py-3 scrollbar-transparent">
      <CursorPanel {store} />
    </Tabs.Content>

    <Tabs.Content value="audio" class="min-h-0 flex-1 overflow-y-auto px-3 py-3 scrollbar-transparent">
      <AudioPanel {store} />
    </Tabs.Content>
  </Tabs.Root>
</aside>
