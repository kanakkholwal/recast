<script lang="ts">
  import type { EditorStore, ExportFormat, ExportQuality } from "$lib/stores/editor-store.svelte";
  import { Check, Upload } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import * as Dialog from "@recast/ui/dialog";
  import { cn } from "@recast/ui/utils";

  interface Props {
    store: EditorStore;
    open: boolean;
    onOpenChange: (open: boolean) => void;
    onConfirm: () => void;
  }

  let { store, open = $bindable(false), onOpenChange, onConfirm }: Props = $props();

  const formats: { value: ExportFormat; label: string; desc: string }[] = [
    { value: "mp4", label: "MP4", desc: "H.264 · universal compatibility" },
    { value: "webm", label: "WebM", desc: "VP9 · web-optimized, smaller" },
    { value: "gif", label: "GIF", desc: "Animated · palette + dither" },
  ];

  const qualities: { value: ExportQuality; label: string; desc: string }[] = [
    { value: "small", label: "Small", desc: "720p · lightest file" },
    { value: "hd", label: "HD", desc: "1080p · balanced" },
    { value: "4k", label: "4K", desc: "2160p · high detail" },
    { value: "source", label: "Source", desc: "Original resolution" },
  ];

  function setFormat(v: ExportFormat) {
    store.exportFormat = v;
  }
  function setQuality(v: ExportQuality) {
    store.exportQuality = v;
  }

  function close() {
    open = false;
    onOpenChange(false);
  }

  function confirm() {
    close();
    // Defer to the next microtask so the dialog dismiss animation has started
    // before the progress UI takes over — prevents a jarring double-overlay.
    queueMicrotask(() => onConfirm());
  }

  function handleKeydown(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      confirm();
    }
  }
</script>

<Dialog.Root bind:open {onOpenChange}>
  <Dialog.Content
    showCloseButton={false}
    class="top-[18%] max-w-md translate-y-0 overflow-hidden rounded-xl p-0 ring-1 ring-border sm:max-w-md"
  >
    <Dialog.Header class="sr-only">
      <Dialog.Title>Export video</Dialog.Title>
      <Dialog.Description>Choose output format and quality.</Dialog.Description>
    </Dialog.Header>

    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      class="flex flex-col focus:outline-none"
      role="document"
      tabindex="-1"
      aria-label="Export settings"
      onkeydown={handleKeydown}
    >
      <header class="flex items-center gap-2 border-b border-border px-4 py-3">
        <div class="flex size-7 items-center justify-center rounded-md border border-primary/30 bg-primary/10 text-primary">
          <Upload size={14} />
        </div>
        <div class="min-w-0 flex-1">
          <h3 class="text-[13px] font-semibold tracking-tight text-foreground">Export video</h3>
          <p class="text-[11px] text-muted-foreground">
            Choose format and quality before saving to disk.
          </p>
        </div>
      </header>

      <section class="flex flex-col gap-2 px-4 pt-4">
        <div class="flex items-center justify-between">
          <span class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
            Format
          </span>
          <span class="font-mono text-[10px] tabular-nums text-muted-foreground/70">
            {store.exportFormat.toUpperCase()}
          </span>
        </div>
        <div class="grid grid-cols-3 gap-1.5">
          {#each formats as fmt (fmt.value)}
            {@const selected = store.exportFormat === fmt.value}
            <button
              type="button"
              onclick={() => setFormat(fmt.value)}
              aria-pressed={selected}
              class={cn(
                "group flex flex-col items-start gap-0.5 rounded-md border px-2.5 py-2 text-left transition-colors",
                selected
                  ? "border-primary bg-primary/10"
                  : "border-border bg-card/40 hover:border-border/80 hover:bg-muted/40",
              )}
            >
              <div class="flex w-full items-center justify-between gap-1">
                <span class={cn("text-[12px] font-semibold", selected ? "text-primary" : "text-foreground")}>
                  {fmt.label}
                </span>
                {#if selected}
                  <Check size={11} class="text-primary" />
                {/if}
              </div>
              <span class="text-[10px] leading-tight text-muted-foreground">{fmt.desc}</span>
            </button>
          {/each}
        </div>
      </section>

      <section class="flex flex-col gap-2 px-4 pb-4 pt-3">
        <div class="flex items-center justify-between">
          <span class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
            Quality
          </span>
          <span class="font-mono text-[10px] tabular-nums text-muted-foreground/70">
            {store.exportQuality.toUpperCase()}
          </span>
        </div>
        <div class="grid grid-cols-2 gap-1.5">
          {#each qualities as q (q.value)}
            {@const selected = store.exportQuality === q.value}
            <button
              type="button"
              onclick={() => setQuality(q.value)}
              aria-pressed={selected}
              class={cn(
                "group flex items-center justify-between gap-2 rounded-md border px-2.5 py-2 text-left transition-colors",
                selected
                  ? "border-primary bg-primary/10"
                  : "border-border bg-card/40 hover:border-border/80 hover:bg-muted/40",
              )}
            >
              <div class="flex min-w-0 flex-col gap-0.5">
                <span class={cn("text-[12px] font-medium", selected ? "text-primary" : "text-foreground")}>
                  {q.label}
                </span>
                <span class="truncate text-[10px] leading-tight text-muted-foreground">{q.desc}</span>
              </div>
              {#if selected}
                <Check size={11} class="shrink-0 text-primary" />
              {/if}
            </button>
          {/each}
        </div>
      </section>

      <footer
        class="flex h-10 items-center justify-between gap-2 border-t border-border bg-muted/30 px-3 text-[11px] text-muted-foreground"
      >
        <span class="hidden items-center gap-1 sm:flex">
          <kbd class="rounded border border-border bg-background px-1.5 py-0.5 font-mono">⌘↵</kbd>
          <span>Start export</span>
        </span>
        <div class="flex items-center gap-1.5">
          <Button variant="ghost" size="xs" onclick={close}>Cancel</Button>
          <Button variant="default" size="xs" class="gap-1.5" onclick={confirm}>
            <Upload size={11} />
            Export
          </Button>
        </div>
      </footer>
    </div>
  </Dialog.Content>
</Dialog.Root>
