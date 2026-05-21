<script lang="ts">
  import type {
    EditorStore,
    ExportFormat,
    ExportQuality,
    GifDither,
    GifQuality,
  } from "$lib/stores/editor-store.svelte";
  import {
    Check,
    Circle,
    Film,
    Image as ImageIcon,
    Infinity as InfinityIcon,
    RotateCcw,
    Sparkles,
    Upload,
    Video,
  } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { Kbd } from "@recast/ui/kbd";
  import { cn } from "@recast/ui/utils";
  import { tick } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { fade, fly, scale, slide } from "svelte/transition";
  import { SliderControl } from "@recast/ui/slider-control";

  interface Props {
    store: EditorStore;
    open: boolean;
    onOpenChange: (open: boolean) => void;
    onConfirm: () => void;
  }

  let {
    store,
    open = $bindable(false),
    onOpenChange,
    onConfirm,
  }: Props = $props();

  const formats: {
    value: ExportFormat;
    label: string;
    desc: string;
    icon: typeof Video;
  }[] = [
    {
      value: "mp4",
      label: "MP4",
      desc: "H.264 · universal",
      icon: Video,
    },
    {
      value: "webm",
      label: "WebM",
      desc: "VP9 · web-optimized",
      icon: Film,
    },
    {
      value: "gif",
      label: "GIF",
      desc: "Animated · palette",
      icon: ImageIcon,
    },
  ];

  const qualities: { value: ExportQuality; label: string; desc: string }[] = [
    { value: "small", label: "Small", desc: "720p · lightest" },
    { value: "hd", label: "HD", desc: "1080p · balanced" },
    { value: "4k", label: "4K", desc: "2160p · high detail" },
    { value: "source", label: "Source", desc: "Original resolution" },
  ];

  const gifQualities: {
    value: GifQuality;
    label: string;
    desc: string;
    swatch: string;
  }[] = [
    { value: "low", label: "Lite", desc: "Smallest file", swatch: "from-rose-300 to-rose-500" },
    { value: "medium", label: "Standard", desc: "Best balance", swatch: "from-amber-300 to-amber-500" },
    { value: "high", label: "Vivid", desc: "Richest colors", swatch: "from-emerald-300 to-emerald-500" },
  ];

  const ditherModes: { value: GifDither; label: string; desc: string }[] = [
    { value: "bayer", label: "Smooth", desc: "Soft gradients (recommended)" },
    { value: "sierra2", label: "Detailed", desc: "Best quality, slightly larger" },
    { value: "none", label: "Sharp", desc: "Crisp edges, visible bands" },
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
    queueMicrotask(() => onConfirm());
  }

  function handleKeydown(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === "Escape") {
      e.preventDefault();
      close();
      return;
    }
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      confirm();
    }
  }

  function formatTime(seconds: number) {
    if (!Number.isFinite(seconds) || seconds <= 0) return "0:00.00";
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    const cs = Math.floor((seconds % 1) * 100);
    return `${mins}:${secs.toString().padStart(2, "0")}.${cs.toString().padStart(2, "0")}`;
  }

  const clipEnd = $derived(
    store.trimEnd > 0 ? store.trimEnd : (store.metadata?.duration ?? 0),
  );
  const clipDuration = $derived(Math.max(0, clipEnd - store.trimStart));
  const sourceDuration = $derived(store.metadata?.duration ?? 0);
  const hasTrim = $derived(
    store.trimStart > 0 ||
      (sourceDuration > 0 &&
        store.trimEnd > 0 &&
        store.trimEnd < sourceDuration),
  );

  const isGif = $derived(store.exportFormat === "gif");
  const activeGifQuality = $derived(
    gifQualities.find((g) => g.value === store.gifSettings.quality),
  );
  const activeDither = $derived(
    ditherModes.find((d) => d.value === store.gifSettings.dither),
  );

  // Portal action — same shape PresetPicker uses, so the modal escapes any
  // overflow:hidden ancestor and the transitions can run on a clean layer.
  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return {
      destroy() {
        if (node.parentNode === document.body) {
          document.body.removeChild(node);
        }
      },
    };
  }

  let dialogRef = $state<HTMLDivElement | null>(null);
  $effect(() => {
    if (open) {
      tick().then(() => dialogRef?.focus());
    }
  });

  function setLoop(value: "infinite" | "once" | number) {
    store.updateGifSettings({ loop: value });
  }
  function setGifQuality(value: GifQuality) {
    store.updateGifSettings({ quality: value });
  }
  function setDither(value: GifDither) {
    store.updateGifSettings({ dither: value });
  }
  function clearFpsOverride() {
    store.updateGifSettings({ fps: null });
  }

  // Loop count cycles through 1..5 when the user clicks the numeric chip.
  function cycleLoopCount() {
    const cur = store.gifSettings.loop;
    const next = typeof cur === "number" ? (cur >= 5 ? 1 : cur + 1) : 1;
    setLoop(next);
  }
</script>

{#if open}
  <div
    use:portal
    class="fixed inset-0 z-100 flex items-start justify-center bg-background/60 px-4 pt-[10vh] backdrop-blur-sm"
    role="presentation"
    onpointerdown={(e) => {
      if (e.target === e.currentTarget) close();
    }}
    in:fade={{ duration: 140 }}
    out:fade={{ duration: 110 }}
  >
    <div
      bind:this={dialogRef}
      role="dialog"
      aria-modal="true"
      aria-labelledby="export-dialog-title"
      onkeydown={handleKeydown}
      tabindex="-1"
      in:scale={{ duration: 200, start: 0.96, easing: cubicOut }}
      out:scale={{ duration: 130, start: 0.97 }}
      class="flex w-full max-w-xl flex-col overflow-hidden rounded-2xl border border-border/60 bg-popover/95 shadow-2xl ring-1 ring-border/40 backdrop-blur-xl focus:outline-none"
    >
      <!-- Header -->
      <header
        in:fly={{ y: -6, duration: 220, delay: 30, easing: cubicOut }}
        class="relative flex items-center gap-3 border-b border-border/60 px-4 py-3.5"
      >
        <div
          class="flex size-9 items-center justify-center rounded-xl border border-primary/30 bg-primary/10 text-primary shadow-(--shadow-craft-inset)"
        >
          <Upload size={15} />
        </div>
        <div class="min-w-0 flex-1">
          <span
            class="inline-flex items-center gap-1 text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground/70"
          >
            <Sparkles size={9} />
            Export
          </span>
          <h3
            id="export-dialog-title"
            class="text-[14px] font-semibold tracking-tight text-foreground"
          >
            Save your recording
          </h3>
        </div>
      </header>

      <!-- Stat strip -->
      <section
        in:fly={{ y: 8, duration: 240, delay: 70, easing: cubicOut }}
        class="grid grid-cols-2 gap-2 border-b border-border/60 bg-muted/15 px-4 py-3"
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
            Range
          </p>
          <p class="mt-1 font-mono text-[12px] tabular-nums text-foreground">
            {formatTime(store.trimStart)} – {formatTime(clipEnd)}
          </p>
        </div>
        {#if hasTrim}
          <p
            class="col-span-2 text-[10px] text-muted-foreground"
            in:fade={{ duration: 200, delay: 200 }}
          >
            Source length:
            <span class="font-mono tabular-nums text-foreground">
              {formatTime(sourceDuration)}
            </span>
          </p>
        {/if}
      </section>

      <!-- Format -->
      <section
        in:fly={{ y: 8, duration: 240, delay: 110, easing: cubicOut }}
        class="flex flex-col gap-2 px-4 pt-4"
      >
        <div class="flex items-center justify-between">
          <span
            class="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground/70"
          >
            Format
          </span>
          <span
            class="font-mono text-[10px] font-semibold tabular-nums text-foreground/80"
          >
            {store.exportFormat.toUpperCase()}
          </span>
        </div>
        <div class="grid grid-cols-3 gap-1.5">
          {#each formats as fmt, i (fmt.value)}
            {@const selected = store.exportFormat === fmt.value}
            {@const Icon = fmt.icon}
            <span
              class="flex"
              in:scale={{ start: 0.92, duration: 220, delay: 140 + i * 35, easing: cubicOut }}
            >
              <button
                type="button"
                onclick={() => setFormat(fmt.value)}
                aria-pressed={selected}
                class={cn(
                  "group flex w-full flex-col items-start gap-1 rounded-xl border px-2.5 py-2 text-left transition-all duration-150",
                  selected
                    ? "border-primary/50 bg-primary/8 ring-1 ring-primary/30 shadow-(--shadow-craft-inset)"
                    : "border-border/40 bg-muted/30 hover:-translate-y-0.5 hover:border-border hover:bg-card",
                )}
              >
                <div class="flex w-full items-center justify-between gap-1">
                  <span
                    class={cn(
                      "flex items-center gap-1.5 text-[12px] font-semibold",
                      selected ? "text-primary" : "text-foreground",
                    )}
                  >
                    <Icon size={11} />
                    {fmt.label}
                  </span>
                  {#if selected}
                    <span in:scale={{ start: 0.5, duration: 180, easing: cubicOut }}>
                      <Check size={11} class="text-primary" />
                    </span>
                  {/if}
                </div>
                <span class="text-[10px] leading-tight text-muted-foreground">
                  {fmt.desc}
                </span>
              </button>
            </span>
          {/each}
        </div>
      </section>

      <!-- Quality -->
      <section
        in:fly={{ y: 8, duration: 240, delay: 170, easing: cubicOut }}
        class="flex flex-col gap-2 px-4 pt-3"
      >
        <div class="flex items-center justify-between">
          <span
            class="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground/70"
          >
            Quality
          </span>
          <span
            class="font-mono text-[10px] font-semibold tabular-nums text-foreground/80"
          >
            {store.exportQuality.toUpperCase()}
          </span>
        </div>
        <div class="grid grid-cols-2 gap-1.5">
          {#each qualities as q, i (q.value)}
            {@const selected = store.exportQuality === q.value}
            <span
              class="flex"
              in:scale={{ start: 0.92, duration: 220, delay: 200 + i * 35, easing: cubicOut }}
            >
              <button
                type="button"
                onclick={() => setQuality(q.value)}
                aria-pressed={selected}
                class={cn(
                  "group flex w-full items-center justify-between gap-2 rounded-xl border px-2.5 py-2 text-left transition-all duration-150",
                  selected
                    ? "border-primary/50 bg-primary/8 ring-1 ring-primary/30 shadow-(--shadow-craft-inset)"
                    : "border-border/40 bg-muted/30 hover:-translate-y-0.5 hover:border-border hover:bg-card",
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
            </span>
          {/each}
        </div>
      </section>

      <!-- GIF settings panel — slides + fades in/out when format toggles -->
      {#if isGif}
        <section
          in:slide={{ duration: 240, easing: cubicOut }}
          out:slide={{ duration: 180, easing: cubicOut }}
          class="overflow-hidden"
        >
          <div
            in:fade={{ duration: 220, delay: 80 }}
            class="mx-4 mt-3 mb-1 flex flex-col gap-3 rounded-xl border border-primary/20 bg-primary/[0.03] p-3 ring-1 ring-primary/10"
          >
            <div class="flex items-center justify-between">
              <span
                class="inline-flex items-center gap-1.5 text-[10px] font-bold uppercase tracking-[0.18em] text-primary"
              >
                <ImageIcon size={10} />
                GIF settings
              </span>
              <Button
                variant="ghost"
                size="xs"
                class="gap-1 text-[10px] text-muted-foreground hover:text-foreground"
                onclick={() =>
                  store.updateGifSettings({
                    fps: null,
                    quality: "medium",
                    loop: "infinite",
                    dither: "bayer",
                  })}
                title="Reset GIF defaults"
              >
                <RotateCcw size={10} />
                Reset
              </Button>
            </div>

            <!-- Frame rate -->
            <div class="flex flex-col gap-1">
              <SliderControl
                label="Frame rate"
                value={store.gifSettings.fps ?? 15}
                min={6}
                max={30}
                step={1}
                unit=" fps"
                description={store.gifSettings.fps === null
                  ? "Auto — follows the quality preset"
                  : undefined}
                onchange={(next) => store.updateGifSettings({ fps: next })}
              >
                {#snippet icon()}
                  <Film size={11} />
                {/snippet}
              </SliderControl>
              {#if store.gifSettings.fps !== null}
                <div class="flex justify-end" in:fade={{ duration: 140 }}>
                  <Button
                    variant="ghost"
                    size="xs"
                    class="text-[10px] text-muted-foreground hover:text-foreground"
                    onclick={clearFpsOverride}
                    title="Use the quality preset's default fps"
                  >
                    <RotateCcw size={10} />
                    Use auto
                  </Button>
                </div>
              {/if}
            </div>

            <!-- Quality + dither row -->
            <div class="grid grid-cols-2 gap-2">
              <div class="flex flex-col gap-1.5">
                <span
                  class="text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
                  title="More colors = richer image, larger file"
                >
                  Color richness
                </span>
                <div class="flex gap-1">
                  {#each gifQualities as gq, i (gq.value)}
                    {@const sel = store.gifSettings.quality === gq.value}
                    <span
                      class="flex flex-1"
                      in:scale={{ start: 0.92, duration: 200, delay: 60 + i * 30, easing: cubicOut }}
                    >
                      <button
                        type="button"
                        onclick={() => setGifQuality(gq.value)}
                        aria-pressed={sel}
                        title={gq.desc}
                        class={cn(
                          "group flex w-full flex-col items-center gap-0.5 rounded-md border px-1 py-1.5 transition-all duration-150",
                          sel
                            ? "border-primary/50 bg-primary/10 ring-1 ring-primary/30"
                            : "border-border/40 bg-background/40 hover:border-border",
                        )}
                      >
                        <span
                          class={cn(
                            "h-1.5 w-full rounded-full bg-gradient-to-r",
                            gq.swatch,
                            !sel && "opacity-60",
                          )}
                        ></span>
                        <span
                          class={cn(
                            "text-[10px] font-semibold",
                            sel ? "text-primary" : "text-foreground",
                          )}
                        >
                          {gq.label}
                        </span>
                      </button>
                    </span>
                  {/each}
                </div>
              </div>

              <div class="flex flex-col gap-1.5">
                <span
                  class="text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
                  title="How smoothly colors blend in gradients"
                >
                  Gradients
                </span>
                <div class="flex gap-1">
                  {#each ditherModes as dm, i (dm.value)}
                    {@const sel = store.gifSettings.dither === dm.value}
                    <span
                      class="flex flex-1"
                      in:scale={{ start: 0.92, duration: 200, delay: 100 + i * 30, easing: cubicOut }}
                    >
                      <button
                        type="button"
                        onclick={() => setDither(dm.value)}
                        aria-pressed={sel}
                        title={dm.desc}
                        class={cn(
                          "w-full rounded-md border px-1.5 py-1.5 text-[10px] font-semibold transition-all duration-150",
                          sel
                            ? "border-primary/50 bg-primary/10 text-primary ring-1 ring-primary/30"
                            : "border-border/40 bg-background/40 text-foreground hover:border-border",
                        )}
                      >
                        {dm.label}
                      </button>
                    </span>
                  {/each}
                </div>
              </div>
            </div>

            <!-- Plain-English caption that mirrors the active richness + gradient
                 selection, so the user always sees what their choices mean. -->
            <p
              class="-mt-1 text-[10px] leading-snug text-muted-foreground"
              aria-live="polite"
            >
              {activeGifQuality?.desc ?? ""}
              <span class="text-muted-foreground/50">·</span>
              {activeDither?.desc ?? ""}
            </p>

            <!-- Loop -->
            <div class="flex flex-col gap-1.5">
              <span
                class="text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
              >
                Loop
              </span>
              <div class="flex items-center gap-1">
                <button
                  type="button"
                  onclick={() => setLoop("infinite")}
                  aria-pressed={store.gifSettings.loop === "infinite"}
                  class={cn(
                    "group flex flex-1 items-center justify-center gap-1.5 rounded-md border px-2 py-1.5 text-[11px] font-medium transition-all duration-150",
                    store.gifSettings.loop === "infinite"
                      ? "border-primary/50 bg-primary/10 text-primary ring-1 ring-primary/30"
                      : "border-border/40 bg-background/40 text-foreground hover:border-border",
                  )}
                >
                  <InfinityIcon size={12} />
                  Forever
                </button>
                <button
                  type="button"
                  onclick={() => setLoop("once")}
                  aria-pressed={store.gifSettings.loop === "once"}
                  class={cn(
                    "group flex flex-1 items-center justify-center gap-1.5 rounded-md border px-2 py-1.5 text-[11px] font-medium transition-all duration-150",
                    store.gifSettings.loop === "once"
                      ? "border-primary/50 bg-primary/10 text-primary ring-1 ring-primary/30"
                      : "border-border/40 bg-background/40 text-foreground hover:border-border",
                  )}
                >
                  <Circle size={11} />
                  Once
                </button>
                <button
                  type="button"
                  onclick={cycleLoopCount}
                  aria-pressed={typeof store.gifSettings.loop === "number"}
                  title="Click to cycle 1× → 2× → … → 5×"
                  class={cn(
                    "group flex flex-1 items-center justify-center gap-1 rounded-md border px-2 py-1.5 font-mono text-[11px] tabular-nums transition-all duration-150",
                    typeof store.gifSettings.loop === "number"
                      ? "border-primary/50 bg-primary/10 text-primary ring-1 ring-primary/30"
                      : "border-border/40 bg-background/40 text-foreground hover:border-border",
                  )}
                >
                  {typeof store.gifSettings.loop === "number"
                    ? `${store.gifSettings.loop}×`
                    : "N×"}
                </button>
              </div>
            </div>
          </div>
        </section>
      {/if}

      <div class="px-4 pb-4 pt-3"></div>

      <!-- Footer -->
      <footer
        in:fly={{ y: 6, duration: 240, delay: 240, easing: cubicOut }}
        class="flex h-11 items-center justify-between gap-2 border-t border-border/60 bg-muted/20 px-3 text-[11px] text-muted-foreground"
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
            Export {store.exportFormat.toUpperCase()}
          </Button>
        </div>
      </footer>
    </div>
  </div>
{/if}
