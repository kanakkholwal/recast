<script lang="ts">
  import * as Tabs from "$components/ui/tabs";
  import * as Tooltip from "$components/ui/tooltip";
  import type { EditorStore } from "$lib/stores/editor-store.svelte";
  import { ImageIcon, MousePointer, Volume2 } from "@lucide/svelte";
  import AudioPanel from "./AudioPanel.svelte";
  import BackgroundPicker from "./BackgroundPicker.svelte";
  import CursorPanel from "./CursorPanel.svelte";

  interface Props {
    store: EditorStore;
  }

  type PanelTab = "background" | "cursor" | "audio";

  type PanelDefinition = {
    id: PanelTab;
    label: string;
    icon: typeof ImageIcon;
  };

  const tabs: PanelDefinition[] = [
    { id: "background", label: "Background", icon: ImageIcon },
    { id: "cursor", label: "Cursor", icon: MousePointer },
    { id: "audio", label: "Audio", icon: Volume2 },
  ];

  let { store }: Props = $props();

  function formatDuration(seconds: number | undefined) {
    if (!seconds || seconds <= 0) return "--:--";
    const totalSeconds = Math.round(seconds);
    const minutes = Math.floor(totalSeconds / 60);
    const remainderSeconds = totalSeconds % 60;
    return `${minutes}:${remainderSeconds.toString().padStart(2, "0")}`;
  }

  function formatResolution() {
    if (!store.metadata?.width || !store.metadata?.height) return "Unknown";
    return `${store.metadata.width} x ${store.metadata.height}`;
  }

  function formatFrameRate() {
    if (!store.metadata?.fps) return "--";
    return `${Math.round(store.metadata.fps)} fps`;
  }
</script>

<div class="flex flex-col h-full bg-card">
  <!-- Metadata badges -->
  <div class="shrink-0 flex items-center gap-1.5 px-3 py-2 border-b border-border/60">
    <div class="rounded-full border border-border/60 bg-background/70 px-2 py-0.5 text-[10px] font-medium text-muted-foreground">
      {formatDuration(store.metadata?.duration)}
    </div>
    <div class="rounded-full border border-border/60 bg-background/70 px-2 py-0.5 text-[10px] font-medium text-muted-foreground">
      {formatResolution()}
    </div>
    <div class="rounded-full border border-border/60 bg-background/70 px-2 py-0.5 text-[10px] font-medium text-muted-foreground">
      {formatFrameRate()}
    </div>
  </div>

  <!-- Icon tab bar -->
  <Tabs.Root value={tabs[0].id} class="flex flex-col flex-1 min-h-0">
    <div class="shrink-0 flex items-center gap-0.5 px-2 py-1.5 border-b border-border/60">
      <Tabs.List class="bg-transparent p-0 h-auto gap-0.5">
        {#each tabs as tab}
          {@const Icon = tab.icon}
          <Tooltip.Root>
            <Tooltip.Trigger>
              <Tabs.Trigger
                value={tab.id}
                class="h-8 w-8 p-0 flex items-center justify-center rounded-md data-[state=active]:bg-muted data-[state=active]:text-foreground text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors"
              >
                <Icon size={16} />
              </Tabs.Trigger>
            </Tooltip.Trigger>
            <Tooltip.Content>{tab.label}</Tooltip.Content>
          </Tooltip.Root>
        {/each}
      </Tabs.List>
    </div>

    <Tabs.Content
      value="background"
      class="custom-scrollbar min-h-0 flex-1 overflow-y-auto px-4 py-3"
    >
      <BackgroundPicker {store} />
    </Tabs.Content>

    <Tabs.Content
      value="cursor"
      class="custom-scrollbar min-h-0 flex-1 overflow-y-auto px-4 py-3"
    >
      <CursorPanel {store} />
    </Tabs.Content>

    <Tabs.Content
      value="audio"
      class="custom-scrollbar min-h-0 flex-1 overflow-y-auto px-4 py-3"
    >
      <AudioPanel {store} />
    </Tabs.Content>
  </Tabs.Root>
</div>

<style>
  .custom-scrollbar::-webkit-scrollbar {
    width: 6px;
  }

  .custom-scrollbar::-webkit-scrollbar-track {
    background: transparent;
  }

  .custom-scrollbar::-webkit-scrollbar-thumb {
    background: rgba(120, 120, 128, 0.35);
    border-radius: 999px;
  }

  .custom-scrollbar {
    scrollbar-width: thin;
    scrollbar-color: rgba(120, 120, 128, 0.35) transparent;
  }
</style>
