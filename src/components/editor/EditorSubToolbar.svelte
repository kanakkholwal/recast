<script lang="ts">
  import * as DropdownMenu from "$components/ui/dropdown-menu";

  import type {
    EditorStore,
    ExportFormat,
    ExportQuality,
  } from "$lib/stores/editor-store.svelte";
  import { ChevronDown, Crop, LayoutGrid } from "@lucide/svelte";

  interface Props {
    store: EditorStore;
  }

  let { store }: Props = $props();

  let showFormatMenu = $state(false);
  let showQualityMenu = $state(false);

  const formats: { value: ExportFormat; label: string; desc: string }[] = [
    { value: "mp4", label: "MP4", desc: "Best quality, universal" },
    { value: "webm", label: "WebM", desc: "Web-optimized, smaller" },
    { value: "gif", label: "GIF", desc: "Animated, shareable" },
  ];

  const exportQualities: { value: ExportQuality; label: string; desc: string }[] = [
    { value: "small", label: "Small", desc: "Up to 720p" },
    { value: "hd", label: "HD", desc: "Up to 1080p" },
    { value: "4k", label: "4K", desc: "Up to 2160p" },
    { value: "source", label: "Source", desc: "Original resolution" },
  ];
</script>

<div class="h-9 flex items-center justify-between px-3 border-b border-border/60 bg-card/30 backdrop-blur-sm shrink-0">
  <!-- Left: Layout mode -->
  <div class="flex items-center gap-1 rounded-md bg-muted/40 p-0.5 border border-border/40">
    <button
      onclick={() => (store.layoutMode = "auto")}
      class="flex items-center gap-1 rounded-[5px] px-2 py-0.5 text-[11px] font-medium transition-all duration-200
        {store.layoutMode === 'auto'
        ? 'bg-background text-foreground shadow-sm'
        : 'text-muted-foreground hover:text-foreground'}"
    >
      <LayoutGrid size={12} />
      Auto
    </button>
    <button
      onclick={() => (store.layoutMode = "crop")}
      class="flex items-center gap-1 rounded-[5px] px-2 py-0.5 text-[11px] font-medium transition-all duration-200
        {store.layoutMode === 'crop'
        ? 'bg-background text-foreground shadow-sm'
        : 'text-muted-foreground hover:text-foreground'}"
    >
      <Crop size={12} />
      Crop
    </button>
  </div>

  <!-- Right: Format + Quality -->
  <div class="flex items-center gap-1.5">
    <DropdownMenu.Root bind:open={showFormatMenu}>
      <DropdownMenu.Trigger
        class="flex items-center gap-1 rounded-md border border-border/50 px-2 py-1 text-[11px] font-medium text-muted-foreground transition-all duration-200 hover:bg-muted hover:text-foreground"
      >
        {store.exportFormat.toUpperCase()}
        <ChevronDown size={10} class="transition-transform duration-200 {showFormatMenu ? 'rotate-180' : ''}" />
      </DropdownMenu.Trigger>
      <DropdownMenu.Content align="end" class="w-48" preventScroll={false}>
        {#each formats as fmt}
          <DropdownMenu.Item
            onclick={() => { store.exportFormat = fmt.value; showFormatMenu = false; }}
            class="flex w-full flex-col items-start gap-0 rounded-md px-3 py-1.5 transition-colors hover:bg-muted {store.exportFormat === fmt.value ? 'bg-muted/50' : ''}"
          >
            <span class="text-xs font-semibold text-foreground">{fmt.label}</span>
            <span class="text-[10px] text-muted-foreground">{fmt.desc}</span>
          </DropdownMenu.Item>
        {/each}
      </DropdownMenu.Content>
    </DropdownMenu.Root>

    <DropdownMenu.Root bind:open={showQualityMenu}>
      <DropdownMenu.Trigger
        class="flex items-center gap-1 rounded-md border border-border/50 px-2 py-1 text-[11px] font-medium text-muted-foreground transition-all duration-200 hover:bg-muted hover:text-foreground"
      >
        {store.exportQuality.toUpperCase()}
        <ChevronDown size={10} class="transition-transform duration-200 {showQualityMenu ? 'rotate-180' : ''}" />
      </DropdownMenu.Trigger>
      <DropdownMenu.Content align="end" class="w-52" preventScroll={false}>
        {#each exportQualities as quality}
          <DropdownMenu.Item
            onclick={() => { store.exportQuality = quality.value; showQualityMenu = false; }}
            class="flex w-full flex-col items-start gap-0 rounded-md px-3 py-1.5 transition-colors hover:bg-muted {store.exportQuality === quality.value ? 'bg-muted/50' : ''}"
          >
            <span class="text-xs font-semibold text-foreground">{quality.label}</span>
            <span class="text-[10px] text-muted-foreground">{quality.desc}</span>
          </DropdownMenu.Item>
        {/each}
      </DropdownMenu.Content>
    </DropdownMenu.Root>
  </div>
</div>
