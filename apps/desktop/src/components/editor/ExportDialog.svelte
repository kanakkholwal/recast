<script lang="ts">
  import type { EditorStore, ExportFormat, ExportQuality } from "$lib/stores/editor-store.svelte";
  import { Check, Upload } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import * as Dialog from "@recast/ui/dialog";
  import { Kbd } from "@recast/ui/kbd";
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

  function formatTime(seconds: number) {
    if (!Number.isFinite(seconds) || seconds <= 0) return "0:00.00";
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    const centiseconds = Math.floor((seconds % 1) * 100);
    return `${mins}:${secs.toString().padStart(2, "0")}.${centiseconds.toString().padStart(2, "0")}`;
  }

  const clipEnd = $derived(store.trimEnd > 0 ? store.trimEnd : (store.metadata?.duration ?? 0));
  const clipDuration = $derived(Math.max(0, clipEnd - store.trimStart));
  const sourceDuration = $derived(store.metadata?.duration ?? 0);
  const hasTrim = $derived(
    store.trimStart > 0 ||
      (sourceDuration > 0 && store.trimEnd > 0 && store.trimEnd < sourceDuration),
  );
</script>

<Dialog.Root bind:open {onOpenChange}>
  <Dialog.Content
    showCloseButton={false}
    class="top-[14%] max-w-md translate-y-0 overflow-hidden rounded-2xl border border-border/60 bg-popover/95 p-0 ring-1 ring-border/40 shadow-2xl backdrop-blur-xl sm:max-w-md"
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
      <header
        class="flex items-center gap-2.5 border-b border-border/60 px-4 py-3"
      >
        <div
          class="flex size-8 items-center justify-center rounded-lg border border-primary/30 bg-primary/10 text-primary shadow-(--shadow-craft-inset)"
        >
          <Upload size={14} />
        </div>
        <div class="min-w-0 flex-1">
          <span
            class="inline-flex items-center gap-1 text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
          >
            Export
          </span>
          <h3
            class="text-[14px] font-semibold tracking-tight text-foreground"
          >
            Save your recording
          </h3>
        </div>
      </header>

      <!-- Stat strip -->
      <section
        class="grid grid-cols-2 gap-2 border-b border-border/60 bg-muted/20 px-4 py-3"
      >
        <div
          class="overflow-hidden rounded-xl border border-border/60 bg-card/70 px-3 py-2 shadow-(--shadow-craft-inset) backdrop-blur"
        >
          <p
            class="text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
          >
            Clip length
          </p>
          <p class="mt-1 font-mono text-[12px] tabular-nums text-foreground">
            {formatTime(clipDuration)}
          </p>
        </div>
        <div
          class="overflow-hidden rounded-xl border border-border/60 bg-card/70 px-3 py-2 shadow-(--shadow-craft-inset) backdrop-blur"
        >
          <p
            class="text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
          >
            Export range
          </p>
          <p class="mt-1 font-mono text-[12px] tabular-nums text-foreground">
            {formatTime(store.trimStart)} – {formatTime(clipEnd)}
          </p>
        </div>
        {#if hasTrim}
          <p class="col-span-2 text-[10px] text-muted-foreground">
            Source length:
            <span class="font-mono tabular-nums text-foreground"
              >{formatTime(sourceDuration)}</span
            >
          </p>
        {/if}
      </section>

      <!-- Format -->
      <section class="flex flex-col gap-2 px-4 pt-4">
        <div class="flex items-center justify-between">
          <span
            class="text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
          >
            Format
          </span>
          <span
            class="font-mono text-[10px] font-semibold tabular-nums text-foreground"
          >
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
                "group flex flex-col items-start gap-0.5 rounded-lg border px-2.5 py-2 text-left transition-all duration-150",
                selected
                  ? "border-primary/40 bg-primary/10 shadow-(--shadow-craft-inset)"
                  : "border-border/40 bg-muted/40 hover:border-border/60 hover:bg-card",
              )}
            >
              <div class="flex w-full items-center justify-between gap-1">
                <span
                  class={cn(
                    "text-[12px] font-semibold",
                    selected ? "text-primary" : "text-foreground",
                  )}
                >
                  {fmt.label}
                </span>
                {#if selected}
                  <Check size={11} class="text-primary" />
                {/if}
              </div>
              <span class="text-[10px] leading-tight text-muted-foreground">
                {fmt.desc}
              </span>
            </button>
          {/each}
        </div>
      </section>

      <!-- Quality -->
      <section class="flex flex-col gap-2 px-4 pb-4 pt-3">
        <div class="flex items-center justify-between">
          <span
            class="text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
          >
            Quality
          </span>
          <span
            class="font-mono text-[10px] font-semibold tabular-nums text-foreground"
          >
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
                "group flex items-center justify-between gap-2 rounded-lg border px-2.5 py-2 text-left transition-all duration-150",
                selected
                  ? "border-primary/40 bg-primary/10 shadow-(--shadow-craft-inset)"
                  : "border-border/40 bg-muted/40 hover:border-border/60 hover:bg-card",
              )}
            >
              <div class="flex min-w-0 flex-col gap-0.5">
                <span
                  class={cn(
                    "text-[12px] font-semibold",
                    selected ? "text-primary" : "text-foreground",
                  )}
                >
                  {q.label}
                </span>
                <span
                  class="truncate text-[10px] leading-tight text-muted-foreground"
                >
                  {q.desc}
                </span>
              </div>
              {#if selected}
                <Check size={11} class="shrink-0 text-primary" />
              {/if}
            </button>
          {/each}
        </div>
      </section>

      <footer
        class="flex h-10 items-center justify-between gap-2 border-t border-border/60 bg-muted/30 px-3 text-[11px] text-muted-foreground"
      >
        <span class="hidden items-center gap-1.5 sm:flex">
          <Kbd>⌘↵</Kbd>
          <span>Start export</span>
        </span>
        <div class="flex items-center gap-1.5">
          <Button variant="ghost" size="xs" onclick={close}>Cancel</Button>
          <Button
            variant="default"
            size="xs"
            class="gap-1.5"
            onclick={confirm}
          >
            <Upload size={11} />
            Export
          </Button>
        </div>
      </footer>
    </div>
  </Dialog.Content>
</Dialog.Root>
