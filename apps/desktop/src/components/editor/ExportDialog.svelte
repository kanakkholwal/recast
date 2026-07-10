<script lang="ts">
  import type {
    EditorStore,
    ExportFormat,
    ExportQuality,
    ExportSpeed,
    GifDither,
    GifQuality,
  } from "$lib/stores/editor-store.svelte";
  import { clockCentis as formatTime } from "$lib/format/time";
  import {
    buildFpsOptions,
    clampSourceFps,
    computeExportDurations,
    computeRemovedDuration,
    nextLoopCount,
  } from "./export-dialog.logic";
  import {
    Check,
    Circle,
    Film,
    Image as ImageIcon,
    Infinity as InfinityIcon,
    RotateCcw,
    Upload,
    Video,
  } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { SliderControl } from "@recast/ui/slider-control";
  import { cn } from "@recast/ui/utils";
  import { cubicOut } from "svelte/easing";
  import { fade, fly, scale } from "svelte/transition";

  interface Props {
    store: EditorStore;
    onConfirm: () => void;
    onCancel: () => void;
  }

  let { store, onConfirm, onCancel }: Props = $props();

  const formats: {
    value: ExportFormat;
    label: string;
    desc: string;
    icon: typeof Video;
  }[] = [
    { value: "mp4", label: "MP4", desc: "Universal", icon: Video },
    { value: "webm", label: "WebM", desc: "Web · VP9", icon: Film },
    { value: "gif", label: "GIF", desc: "Animated", icon: ImageIcon },
  ];

  const qualities: { value: ExportQuality; label: string; desc: string }[] = [
    { value: "small", label: "Small", desc: "720p · lightest" },
    { value: "hd", label: "HD", desc: "1080p · balanced" },
    { value: "4k", label: "4K", desc: "2160p · high detail" },
    { value: "source", label: "Source", desc: "Original resolution" },
  ];

  // Encoder effort — orthogonal to resolution; trades encode time for file size.
  const speeds: { value: ExportSpeed; label: string; desc: string }[] = [
    { value: "fast", label: "Fast", desc: "Quicker · larger" },
    { value: "balanced", label: "Balanced", desc: "Recommended" },
    { value: "quality", label: "Quality", desc: "Slower · smaller" },
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
  function setSpeed(v: ExportSpeed) {
    store.exportSpeed = v;
  }
  function setFps(v: number | null) {
    store.exportFps = v;
  }

  // Captions only matter once a transcript exists; the section hides otherwise.
  const hasCaptions = $derived(
    !!store.transcript && store.transcript.segments.length > 0,
  );
  const sidecarOptions: {
    value: "none" | "vtt" | "srt";
    label: string;
    desc: string;
  }[] = [
    { value: "none", label: "None", desc: "Skip file" },
    { value: "vtt", label: ".VTT", desc: "Web player" },
    { value: "srt", label: ".SRT", desc: "Universal" },
  ];
  const burnOptions: { value: boolean; label: string; desc: string }[] = [
    { value: true, label: "On", desc: "Baked in" },
    { value: false, label: "Off", desc: "Clean video" },
  ];
  function setBurnIn(v: boolean) {
    store.updateCaptionExport({ burnIn: v });
  }
  function setSidecar(v: "none" | "vtt" | "srt") {
    store.updateCaptionExport({ sidecar: v });
  }

  const sourceFps = $derived(clampSourceFps(store.metadata?.fps));
  const fpsOptions = $derived(buildFpsOptions(sourceFps));
  // Reads the store directly rather than `isGif` (declared below) to avoid a
  // use-before-declaration.
  const showFps = $derived(
    store.exportFormat !== "gif" && fpsOptions.length > 1,
  );


  const clipEnd = $derived(
    store.trimEnd > 0 ? store.trimEnd : (store.metadata?.duration ?? 0),
  );
  const sourceDuration = $derived(store.metadata?.duration ?? 0);
  // `effectiveCuts` already drops opted-off cuts.
  const removedDuration = $derived(
    computeRemovedDuration(store.effectiveCuts, store.trimStart, clipEnd),
  );
  const durations = $derived(
    computeExportDurations(clipEnd, store.trimStart, removedDuration),
  );
  const clipDuration = $derived(durations.clipDuration);
  const outputDuration = $derived(durations.outputDuration);
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

  // Two columns on roomy viewports, single stacked column when tight. The
  // flow-dialog wrapper morphs its chrome to the width this body reports.
  let viewportWidth = $state(
    typeof window !== "undefined" ? window.innerWidth : 1280,
  );
  $effect(() => {
    const onResize = () => (viewportWidth = window.innerWidth);
    onResize();
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  });
  const isCompact = $derived(viewportWidth < 720);
  const bodyWidth = $derived(
    isCompact ? Math.min(460, viewportWidth - 32) : 680,
  );

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

  function cycleLoopCount() {
    setLoop(nextLoopCount(store.gifSettings.loop));
  }

  function resetGifDefaults() {
    store.updateGifSettings({
      fps: null,
      quality: "medium",
      loop: "infinite",
      dither: "bayer",
    });
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      onConfirm();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#snippet sectionLabel(label: string, description?: string)}
  <div class="flex flex-col gap-0.5">
    <span
      class="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground/70"
    >
      {label}
    </span>
    {#if description}
      <span class="text-[11px] text-muted-foreground/80">{description}</span>
    {/if}
  </div>
{/snippet}

{#snippet gifSettingsBody()}
  <div class="flex flex-col gap-4 px-5 py-4">
    <div class="flex items-start justify-between gap-3">
      {@render sectionLabel("GIF settings", "Tune palette, gradients, and loop.")}
      <Button
        variant="ghost"
        size="xs"
        class="h-6 gap-1 px-1.5 text-[10.5px] text-muted-foreground hover:text-foreground"
        onclick={resetGifDefaults}
        title="Reset GIF defaults"
      >
        <RotateCcw class="size-3" />
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
          ? "Auto (follows the quality preset)"
          : undefined}
        onchange={(next: number) => store.updateGifSettings({ fps: next })}
      >
        {#snippet icon()}
          <Film class="size-3" />
        {/snippet}
      </SliderControl>
      {#if store.gifSettings.fps !== null}
        <div class="flex justify-end" in:fade={{ duration: 140 }}>
          <Button
            variant="ghost"
            size="xs"
            class="h-6 gap-1 px-1.5 text-[10.5px] text-muted-foreground hover:text-foreground"
            onclick={clearFpsOverride}
            title="Use the quality preset's default fps"
          >
            <RotateCcw class="size-3" />
            Use auto
          </Button>
        </div>
      {/if}
    </div>

    <!-- Color richness -->
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
                "group flex w-full flex-col items-center gap-1 rounded-md border px-1.5 py-1.5 transition-all duration-200",
                sel
                  ? "border-primary/40 bg-primary/8 ring-1 ring-primary/25"
                  : "border-border/40 bg-card/40 hover:border-border/70 hover:bg-card/70",
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
                  "text-[10.5px] font-semibold",
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

    <!-- Gradients -->
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
                "w-full rounded-md border px-2 py-1.5 text-[10.5px] font-semibold transition-all duration-200",
                sel
                  ? "border-primary/40 bg-primary/8 text-primary ring-1 ring-primary/25"
                  : "border-border/40 bg-card/40 text-foreground hover:border-border/70 hover:bg-card/70",
              )}
            >
              {dm.label}
            </button>
          </span>
        {/each}
      </div>
    </div>

    <p
      class="-mt-2 text-[11px] leading-snug text-muted-foreground/90"
      aria-live="polite"
    >
      {activeGifQuality?.desc ?? ""}
      <span class="text-muted-foreground/40">·</span>
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
            "flex flex-1 items-center justify-center gap-1.5 rounded-md border px-2 py-1.5 text-[11px] font-medium transition-all duration-200",
            store.gifSettings.loop === "infinite"
              ? "border-primary/40 bg-primary/8 text-primary ring-1 ring-primary/25"
              : "border-border/40 bg-card/40 text-foreground hover:border-border/70 hover:bg-card/70",
          )}
        >
          <InfinityIcon class="size-3.5" />
          Forever
        </button>
        <button
          type="button"
          onclick={() => setLoop("once")}
          aria-pressed={store.gifSettings.loop === "once"}
          class={cn(
            "flex flex-1 items-center justify-center gap-1.5 rounded-md border px-2 py-1.5 text-[11px] font-medium transition-all duration-200",
            store.gifSettings.loop === "once"
              ? "border-primary/40 bg-primary/8 text-primary ring-1 ring-primary/25"
              : "border-border/40 bg-card/40 text-foreground hover:border-border/70 hover:bg-card/70",
          )}
        >
          <Circle class="size-3" />
          Once
        </button>
        <button
          type="button"
          onclick={cycleLoopCount}
          aria-pressed={typeof store.gifSettings.loop === "number"}
          title="Click to cycle 1× → 2× → … → 5×"
          class={cn(
            "flex flex-1 items-center justify-center gap-1 rounded-md border px-2 py-1.5 font-mono text-[11px] tabular-nums transition-all duration-200",
            typeof store.gifSettings.loop === "number"
              ? "border-primary/40 bg-primary/8 text-primary ring-1 ring-primary/25"
              : "border-border/40 bg-card/40 text-foreground hover:border-border/70 hover:bg-card/70",
          )}
        >
          {typeof store.gifSettings.loop === "number"
            ? `${store.gifSettings.loop}×`
            : "N×"}
        </button>
      </div>
    </div>
  </div>
{/snippet}

{#snippet formatSection()}
  <section
    in:fly={{ y: 8, duration: 240, delay: 110, easing: cubicOut }}
    class="flex flex-col gap-2.5"
  >
    {@render sectionLabel("Format", "How the file is encoded.")}
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
              "group relative flex w-full flex-col items-start gap-1 rounded-xl border px-3 py-2.5 text-left transition-all duration-200",
              selected
                ? "border-primary/40 bg-primary/8 ring-1 ring-primary/25"
                : "border-border/40 bg-card/40 hover:-translate-y-0.5 hover:border-border/70 hover:bg-card/70 hover:shadow-craft-sm",
            )}
          >
            <span
              class={cn(
                "flex items-center gap-1.5 text-[12.5px] font-semibold tracking-tight",
                selected ? "text-primary" : "text-foreground",
              )}
            >
              <Icon class="size-3.5" />
              {fmt.label}
            </span>
            <span class="text-[10.5px] leading-tight text-muted-foreground">
              {fmt.desc}
            </span>
            {#if selected}
              <span
                class="absolute right-2 top-2"
                in:scale={{ start: 0.5, duration: 180, easing: cubicOut }}
              >
                <Check class="size-3 text-primary" />
              </span>
            {/if}
          </button>
        </span>
      {/each}
    </div>
  </section>
{/snippet}

{#snippet qualitySection()}
  <section
    in:fly={{ y: 8, duration: 240, delay: 170, easing: cubicOut }}
    class="flex flex-col gap-2.5"
  >
    {@render sectionLabel("Quality", "Resolution preset for the export.")}
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
              "group flex w-full items-center justify-between gap-2 rounded-xl border px-3 py-2.5 text-left transition-all duration-200",
              selected
                ? "border-primary/40 bg-primary/8 ring-1 ring-primary/25"
                : "border-border/40 bg-card/40 hover:-translate-y-0.5 hover:border-border/70 hover:bg-card/70 hover:shadow-craft-sm",
            )}
          >
            <div class="flex min-w-0 flex-col gap-0.5">
              <span
                class={cn(
                  "text-[12.5px] font-semibold tracking-tight",
                  selected ? "text-primary" : "text-foreground",
                )}
              >
                {q.label}
              </span>
              <span
                class="truncate text-[10.5px] leading-tight text-muted-foreground"
              >
                {q.desc}
              </span>
            </div>
            {#if selected}
              <Check class="size-3 shrink-0 text-primary" />
            {/if}
          </button>
        </span>
      {/each}
    </div>
  </section>
{/snippet}

{#snippet fpsSection()}
  <!-- Output fps for MP4/WebM. Hidden for GIF (own fps control) and when the
       source is already ≤24 fps (no meaningful choice). -->
  <section
    in:fly={{ y: 8, duration: 240, delay: 185, easing: cubicOut }}
    class="flex flex-col gap-2.5"
  >
    {@render sectionLabel("Frame rate", "Original keeps the source rate.")}
    <div
      class={cn(
        "grid gap-1.5",
        fpsOptions.length === 2 ? "grid-cols-2" : "grid-cols-3",
      )}
    >
      {#each fpsOptions as f, i (f.value ?? "original")}
        {@const selected = store.exportFps === f.value}
        <span
          class="flex"
          in:scale={{ start: 0.92, duration: 220, delay: 215 + i * 35, easing: cubicOut }}
        >
          <button
            type="button"
            onclick={() => setFps(f.value)}
            aria-pressed={selected}
            title={f.desc}
            class={cn(
              "group flex w-full flex-col items-center gap-0.5 rounded-xl border px-2 py-2 text-center transition-all duration-200",
              selected
                ? "border-primary/40 bg-primary/8 ring-1 ring-primary/25"
                : "border-border/40 bg-card/40 hover:-translate-y-0.5 hover:border-border/70 hover:bg-card/70 hover:shadow-craft-sm",
            )}
          >
            <span
              class={cn(
                "text-[12.5px] font-semibold tracking-tight",
                selected ? "text-primary" : "text-foreground",
              )}
            >
              {f.label}
            </span>
            <span
              class="truncate text-[10px] leading-tight text-muted-foreground"
            >
              {f.desc}
            </span>
          </button>
        </span>
      {/each}
    </div>
  </section>
{/snippet}

{#snippet speedSection()}
  <!-- Hidden for GIF, which uses a palette 2-pass and ignores codec presets. -->
  <section
    in:fly={{ y: 8, duration: 240, delay: 200, easing: cubicOut }}
    class="flex flex-col gap-2.5"
  >
    {@render sectionLabel("Speed", "Encoder effort. Same resolution.")}
    <div class="grid grid-cols-3 gap-1.5">
      {#each speeds as s, i (s.value)}
        {@const selected = store.exportSpeed === s.value}
        <span
          class="flex"
          in:scale={{ start: 0.92, duration: 220, delay: 230 + i * 35, easing: cubicOut }}
        >
          <button
            type="button"
            onclick={() => setSpeed(s.value)}
            aria-pressed={selected}
            title={s.desc}
            class={cn(
              "group flex w-full flex-col items-center gap-0.5 rounded-xl border px-2 py-2 text-center transition-all duration-200",
              selected
                ? "border-primary/40 bg-primary/8 ring-1 ring-primary/25"
                : "border-border/40 bg-card/40 hover:-translate-y-0.5 hover:border-border/70 hover:bg-card/70 hover:shadow-craft-sm",
            )}
          >
            <span
              class={cn(
                "text-[12.5px] font-semibold tracking-tight",
                selected ? "text-primary" : "text-foreground",
              )}
            >
              {s.label}
            </span>
            <span
              class="truncate text-[10px] leading-tight text-muted-foreground"
            >
              {s.desc}
            </span>
          </button>
        </span>
      {/each}
    </div>
  </section>
{/snippet}

{#snippet captionsSection()}
  <!-- Only shown once a transcript exists. Two independent choices: burn the
       captions into the pixels, and/or save a sidecar file (the sidecar is also
       what Cloud uploads as a selectable track). Renders full-width below the
       encoding grid; splits into two balanced columns when there's room. -->
  <section
    in:fly={{ y: 8, duration: 240, delay: 215, easing: cubicOut }}
    class="flex flex-col gap-2.5"
  >
    {@render sectionLabel("Captions", "From your transcript, burn in and/or export a file.")}
    <div
      class={cn(
        "grid items-start gap-x-6 gap-y-3",
        !isGif && !isCompact ? "grid-cols-2" : "grid-cols-1",
      )}
    >
      {#if !isGif}
        <div class="flex flex-col gap-1.5">
          {@render captionSubLabel("Burn into video")}
          <div class="grid grid-cols-2 gap-1.5">
            {#each burnOptions as o (o.label)}
              {@render optionButton(
                store.captionExport.burnIn === o.value,
                o.label,
                o.desc,
                () => setBurnIn(o.value),
              )}
            {/each}
          </div>
        </div>
      {/if}
      <div class="flex flex-col gap-1.5">
        {@render captionSubLabel("Separate file")}
        <div class="grid grid-cols-3 gap-1.5">
          {#each sidecarOptions as s (s.value)}
            {@render optionButton(
              store.captionExport.sidecar === s.value,
              s.label,
              s.desc,
              () => setSidecar(s.value),
            )}
          {/each}
        </div>
      </div>
    </div>
  </section>
{/snippet}

{#snippet captionSubLabel(label: string)}
  <span class="text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70">
    {label}
  </span>
{/snippet}

{#snippet optionButton(
  selected: boolean,
  label: string,
  desc: string,
  onclick: () => void,
)}
  <button
    type="button"
    {onclick}
    aria-pressed={selected}
    title={desc}
    class={cn(
      "group flex w-full flex-col items-center gap-0.5 rounded-xl border px-2 py-2 text-center transition-all duration-200",
      selected
        ? "border-primary/40 bg-primary/8 ring-1 ring-primary/25"
        : "border-border/40 bg-card/40 hover:-translate-y-0.5 hover:border-border/70 hover:bg-card/70 hover:shadow-craft-sm",
    )}
  >
    <span
      class={cn(
        "text-[12.5px] font-semibold tracking-tight",
        selected ? "text-primary" : "text-foreground",
      )}
    >
      {label}
    </span>
    <span class="truncate text-[10px] leading-tight text-muted-foreground">{desc}</span>
  </button>
{/snippet}

<div class="flex flex-col" style="width: {bodyWidth}px;">
  <!-- Header -->
  <header
    in:fly={{ y: -6, duration: 220, delay: 30, easing: cubicOut }}
    class="flex items-start gap-3 border-b border-border/40 px-5 py-4"
  >
    <div
      class="flex size-10 items-center justify-center rounded-xl border border-primary/30 bg-primary/10 text-primary shadow-(--shadow-craft-inset)"
    >
      <Upload class="size-4" />
    </div>
    <div class="min-w-0 flex-1 pt-0.5">
      <h3
        id="export-flow-title"
        class="text-[14px] font-semibold tracking-tight text-foreground"
      >
        Export recording
      </h3>
      <p class="mt-0.5 text-[11px] text-muted-foreground">
        Choose a format and quality, then start the export.
      </p>
    </div>
  </header>

  <section
    in:fly={{ y: 8, duration: 240, delay: 70, easing: cubicOut }}
    class="flex items-stretch divide-x divide-border/40 border-b border-border/40 bg-muted/15 px-5 py-2.5"
  >
    <div class="flex flex-1 flex-col gap-0.5 pr-4">
      <span
        class="text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
      >
        Output
      </span>
      <span class="font-mono text-[12px] tabular-nums text-foreground">
        {formatTime(outputDuration)}
      </span>
    </div>
    <div class="flex flex-1 flex-col gap-0.5 pl-4">
      <span
        class="text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
      >
        Range
      </span>
      <span class="font-mono text-[12px] tabular-nums text-foreground">
        {formatTime(store.trimStart)} – {formatTime(clipEnd)}
      </span>
    </div>
  </section>
  {#if removedDuration > 0.05}
    <p
      class="border-b border-border/40 bg-muted/10 px-5 py-1.5 text-[10.5px] text-muted-foreground"
      in:fade={{ duration: 200, delay: 190 }}
    >
      Cuts remove
      <span class="mx-1 font-mono tabular-nums text-foreground">
        {formatTime(removedDuration)}
      </span>
      from the {formatTime(clipDuration)} trimmed clip
    </p>
  {/if}
  {#if hasTrim}
    <p
      class="border-b border-border/40 bg-muted/10 px-5 py-1.5 text-[10.5px] text-muted-foreground"
      in:fade={{ duration: 200, delay: 200 }}
    >
      Source length
      <span class="ml-1 font-mono tabular-nums text-foreground">
        {formatTime(sourceDuration)}
      </span>
    </p>
  {/if}

  <!-- GIF settings, wrapped so it reads as one panel in either layout. -->
  {#snippet gifSettingsCard()}
    <div
    >
      {@render gifSettingsBody()}
    </div>
  {/snippet}

  {#if isCompact}
    <!-- Narrow viewports: a single stacked column. -->
    <div class="flex flex-col gap-4 px-5 py-4">
      {@render formatSection()}
      {@render qualitySection()}
      {#if showFps}{@render fpsSection()}{/if}
      {#if !isGif}{@render speedSection()}{/if}
      {#if isGif}
        <section in:fade={{ duration: 220, delay: 100 }}>
          {@render gifSettingsCard()}
        </section>
      {/if}
      {#if hasCaptions}{@render captionsSection()}{/if}
    </div>
  {:else}
    <!-- Two columns for the encoding knobs; Captions spans full width below so
         the grid stays balanced no matter which caption options are shown. -->
    <div class="flex flex-col gap-5 px-5 py-5">
      <div class="grid grid-cols-2 items-start gap-x-6 gap-y-5">
        <div class="flex min-w-0 flex-col gap-5">
          {@render formatSection()}
          {@render qualitySection()}
        </div>
        <div class="flex min-w-0 flex-col gap-5">
          {#if isGif}
            <section in:fade={{ duration: 220, delay: 160 }}>
              {@render gifSettingsCard()}
            </section>
          {:else}
            {#if showFps}{@render fpsSection()}{/if}
            {@render speedSection()}
          {/if}
        </div>
      </div>
      {#if hasCaptions}
        <div class="border-t border-border/40 pt-5">
          {@render captionsSection()}
        </div>
      {/if}
    </div>
  {/if}

  <!-- Footer -->
  <footer
    in:fly={{ y: 6, duration: 240, delay: 240, easing: cubicOut }}
    class="flex items-center justify-end gap-2 border-t border-border/40 bg-muted/30 px-3 py-2.5"
  >
    <Button variant="ghost" size="xs" onclick={onCancel}>Cancel</Button>
    <Button
      variant="default"
      size="xs"
      class="gap-1.5"
      onclick={onConfirm}
    >
      <Upload class="size-3" />
      Export {store.exportFormat.toUpperCase()}
    </Button>
  </footer>
</div>
