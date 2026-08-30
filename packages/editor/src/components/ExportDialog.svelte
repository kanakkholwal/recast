<script lang="ts" module>
// Survives the export panel's per-phase remount.
let advancedWasOpen = $state(false);
</script>

<script lang="ts">
  import type {
    EditorStore,
    ExportFormat,
    ExportQuality,
    ExportSpeed,
    GifDither,
    GifQuality,
  } from "../stores/editor-store.svelte";
  import { clockCentis as formatTime } from "../lib/format/time";
  import {
    buildFpsOptions,
    clampSourceFps,
    computeExportDurations,
    computeRemovedDuration,
  } from "./export-dialog.logic";
  import { ChevronDown, Film, Minus, Plus, RotateCcw, Settings2, Upload } from "@recast/icons";
  import { Button } from "@recast/ui/button";
  import { Segmented, SegmentedToggle } from "@recast/ui/segmented";
  import { SliderControl } from "@recast/ui/slider-control";
  import { cn } from "@recast/ui/utils";
  import { cubicOut } from "svelte/easing";
  import { fade, slide } from "svelte/transition";
  import { motionDuration } from "../lib/motion.svelte";
  import {
    estimateExportBytes,
    formatByteRange,
    outputResolution,
  } from "./export-estimate";

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
  }[] = [
    { value: "mp4", label: "MP4", desc: "Plays everywhere" },
    { value: "webm", label: "WebM", desc: "VP9, smaller files, for the web" },
    { value: "gif", label: "GIF", desc: "Silent, loops, large" },
  ];

  const qualities: { value: ExportQuality; label: string; desc: string }[] = [
    { value: "small", label: "720p", desc: "Lightest file" },
    { value: "hd", label: "1080p", desc: "Balanced, the usual choice" },
    { value: "4k", label: "4K", desc: "2160p, high detail, large file" },
    { value: "source", label: "Source", desc: "Keeps the original resolution" },
  ];

  // Encoder effort, orthogonal to resolution; trades encode time for file size.
  const speeds: { value: ExportSpeed; label: string; desc: string }[] = [
    { value: "fast", label: "Fast", desc: "Encodes quicker, larger file" },
    { value: "balanced", label: "Balanced", desc: "Recommended" },
    { value: "quality", label: "Quality", desc: "Slower, smallest file" },
  ];

  // A brand-tinted intensity ramp, so it reads as more colour richness, not a good-versus-bad judgement.
  const gifQualities: {
    value: GifQuality;
    label: string;
    desc: string;
    swatch: string;
  }[] = [
    { value: "low", label: "Lite", desc: "Smallest file", swatch: "from-primary/20 to-primary/45" },
    { value: "medium", label: "Standard", desc: "Best balance", swatch: "from-primary/45 to-primary/70" },
    { value: "high", label: "Vivid", desc: "Richest colors", swatch: "from-primary/75 to-primary" },
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
    desc?: string;
  }[] = [
    { value: "none", label: "None" },
    { value: "vtt", label: ".VTT", desc: "Web player" },
    { value: "srt", label: ".SRT", desc: "Universal" },
  ];
  function setBurnIn(v: boolean) {
    store.updateCaptionExport({ burnIn: v });
  }
  function setSidecar(v: "none" | "vtt" | "srt") {
    store.updateCaptionExport({ sidecar: v });
  }

  const sourceFps = $derived(clampSourceFps(store.metadata?.fps));
  const fpsOptions = $derived(buildFpsOptions(sourceFps));
  // Reads the store directly rather than `isGif`, which is declared below.
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

  // The quality preset is a BOUND, so a portrait clip at HD is 608x1080 and the preset label alone misleads.
  const outRes = $derived(
    outputResolution(
      store.metadata?.width ?? 0,
      store.metadata?.height ?? 0,
      store.exportQuality,
    ),
  );
  const effectiveFps = $derived(
    store.exportFormat === "gif"
      ? (store.gifSettings.fps ?? 15)
      : (store.exportFps ?? sourceFps),
  );
  const sizeEstimate = $derived(
    formatByteRange(
      outRes
        ? estimateExportBytes({
            format: store.exportFormat,
            quality: store.exportQuality,
            speed: store.exportSpeed,
            seconds: outputDuration,
            width: outRes.width,
            height: outRes.height,
            fps: effectiveFps,
          })
        : null,
    ),
  );

  const activeFormat = $derived(formats.find((f) => f.value === store.exportFormat));
  const activeQuality = $derived(qualities.find((q) => q.value === store.exportQuality));
  const activeSpeed = $derived(speeds.find((sp) => sp.value === store.exportSpeed));
  const activeFps = $derived(fpsOptions.find((f) => f.value === store.exportFps));
  const activeSidecar = $derived(
    sidecarOptions.find((o) => o.value === store.captionExport.sidecar),
  );

  const isGif = $derived(store.exportFormat === "gif");
  const activeGifQuality = $derived(
    gifQualities.find((g) => g.value === store.gifSettings.quality),
  );
  const activeDither = $derived(
    ditherModes.find((d) => d.value === store.gifSettings.dither),
  );

  // Shown on the collapsed Advanced row so its contents aren't a mystery box.
  const advancedSummary = $derived(
    store.exportFormat === "gif"
      ? [activeGifQuality?.label, activeDither?.label].filter(Boolean).join(" · ")
      : [showFps ? activeFps?.label : null, activeSpeed?.label]
          .filter(Boolean)
          .join(" · "),
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

  // 'Once' already covers a single play, so the range starts at 2; stepping from Forever or Once switches to count.
  const LOOP_MIN = 2;
  const LOOP_MAX = 5;
  const loopCount = $derived(
    typeof store.gifSettings.loop === "number" ? store.gifSettings.loop : null,
  );
  function stepLoop(delta: number) {
    // First press from Forever/Once switches to count mode at the minimum.
    if (loopCount === null) {
      setLoop(LOOP_MIN);
      return;
    }
    setLoop(Math.min(LOOP_MAX, Math.max(LOOP_MIN, loopCount + delta)));
  }

  // Module-level so the disclosure survives the panel's per-phase remount and a Speed-tuner needn't reopen it every export.
  let advancedOpen = $state(advancedWasOpen);
  $effect(() => {
    advancedWasOpen = advancedOpen;
  });

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

{#snippet gifSettingsBody()}
  <div class="flex flex-col gap-4">
    {#snippet gifQualityControl()}
      <Segmented
        options={gifQualities.map((g) => ({ value: g.value, label: g.label }))}
        value={store.gifSettings.quality}
        onValueChange={setGifQuality}
        aria-label="Color richness"
      />
    {/snippet}
    {@render field("Color richness", activeGifQuality?.desc, gifQualityControl)}

    {#snippet ditherControl()}
      <Segmented
        options={ditherModes.map((d) => ({ value: d.value, label: d.label }))}
        value={store.gifSettings.dither}
        onValueChange={setDither}
        aria-label="Gradients"
      />
    {/snippet}
    {@render field("Gradients", activeDither?.desc, ditherControl)}

    {#snippet loopControl()}
      <div class="flex items-center gap-1.5">
        <Segmented
          class="flex-1"
          options={[
            { value: "infinite", label: "Forever" },
            { value: "once", label: "Once" },
            { value: "count", label: loopCount !== null ? `${loopCount}x` : "Count" },
          ]}
          value={loopCount !== null ? "count" : (store.gifSettings.loop as string)}
          onValueChange={(v) =>
            v === "count" ? setLoop(loopCount ?? LOOP_MIN) : setLoop(v as "infinite" | "once")}
          aria-label="Loop"
        />
        {#if loopCount !== null}
          <div class="flex shrink-0 items-center rounded-lg ring-1 ring-inset ring-border/40">
            <button
              type="button"
              onclick={() => stepLoop(-1)}
              disabled={loopCount <= LOOP_MIN}
              aria-label="Fewer loops"
              class="grid size-7 place-items-center rounded-l-lg text-muted-foreground transition-colors hover:text-foreground disabled:pointer-events-none disabled:opacity-40"
            >
              <Minus class="size-3" />
            </button>
            <button
              type="button"
              onclick={() => stepLoop(1)}
              disabled={loopCount >= LOOP_MAX}
              aria-label="More loops"
              class="grid size-7 place-items-center rounded-r-lg text-muted-foreground transition-colors hover:text-foreground disabled:pointer-events-none disabled:opacity-40"
            >
              <Plus class="size-3" />
            </button>
          </div>
        {/if}
      </div>
    {/snippet}
    {@render field("Loop", undefined, loopControl)}

    {#snippet gifFpsControl()}
      <SliderControl
        label="Frame rate"
        value={store.gifSettings.fps ?? 15}
        min={6}
        max={30}
        step={1}
        unit=" fps"
        description={store.gifSettings.fps === null ? "Auto, follows the preset" : undefined}
        onchange={(next: number) => store.updateGifSettings({ fps: next })}
      >
        {#snippet icon()}
          <Film class="size-3" />
        {/snippet}
      </SliderControl>
    {/snippet}
    {@render gifFpsControl()}

    <div class="flex justify-end gap-1">
      {#if store.gifSettings.fps !== null}
        <Button
          variant="ghost"
          size="xs"
          class="text-[11px] text-muted-foreground hover:text-foreground"
          onclick={clearFpsOverride}
        >
          Auto frame rate
        </Button>
      {/if}
      <Button
        variant="ghost"
        size="xs"
        class="gap-1 text-[11px] text-muted-foreground hover:text-foreground"
        onclick={resetGifDefaults}
      >
        <RotateCcw class="size-3" />
        Reset
      </Button>
    </div>
  </div>
{/snippet}

{#snippet field(label: string, desc: string | undefined, control: import("svelte").Snippet)}
  <div class="flex flex-col gap-1.5">
    <span class="text-[11px] font-semibold text-foreground">{label}</span>
    {@render control()}
    {#if desc}
      <p class="text-[11px] leading-snug text-muted-foreground">{desc}</p>
    {/if}
  </div>
{/snippet}

{#snippet captionsSection()}
  <!-- Only shown once a transcript exists. Burning in and writing a sidecar are
       independent: the sidecar is also what Cloud uploads as a selectable track. -->
  <div class="flex flex-col gap-3">
    <div class="flex items-center justify-between gap-3">
      <div class="min-w-0">
        <p class="text-[11px] font-semibold text-foreground">Burn captions in</p>
        <p class="text-[11px] leading-snug text-muted-foreground">
          Viewers can't turn them off.
        </p>
      </div>
      <SegmentedToggle
        checked={store.captionExport.burnIn}
        size="xs"
        aria-label="Burn captions into the video"
        onCheckedChange={setBurnIn}
      />
    </div>
    {#snippet sidecarControl()}
      <Segmented
        options={sidecarOptions.map((o) => ({ value: o.value, label: o.label }))}
        value={store.captionExport.sidecar}
        onValueChange={setSidecar}
        aria-label="Caption file"
      />
    {/snippet}
    {@render field("Caption file", activeSidecar?.desc, sidecarControl)}
  </div>
{/snippet}

<div class="flex h-full min-h-0 flex-col">
  <!-- Pinned header + summary. Stays put while the option list scrolls, so the
       "what am I exporting" anchor is always visible. -->
  <div class="shrink-0">
    <!-- Title and the three facts that answer "what am I about to get" read as
         one block. Three stacked bands (title, tinted stats, trim note) made a
         header taller than the first two controls put together. -->
    <header class="flex flex-col gap-3 border-b border-border/40 px-5 pb-3.5 pt-4">
      <h3
        id="export-flow-title"
        class="text-[15px] font-semibold tracking-tight text-foreground"
      >
        Export recording
      </h3>
      <dl class="grid grid-cols-3 gap-x-3">
        <div class="flex flex-col gap-0.5">
          <dt class="text-[11px] text-muted-foreground">Duration</dt>
          <dd class="font-mono text-[13px] font-medium tabular-nums text-foreground">
            {formatTime(outputDuration)}
          </dd>
        </div>
        <div class="flex flex-col gap-0.5">
          <dt class="text-[11px] text-muted-foreground">Resolution</dt>
          <dd class="font-mono text-[13px] font-medium tabular-nums text-foreground">
            {outRes ? `${outRes.width}×${outRes.height}` : "–"}
          </dd>
        </div>
        <div class="flex flex-col gap-0.5">
          <dt class="text-[11px] text-muted-foreground">Est. size</dt>
          <dd class="font-mono text-[13px] font-medium tabular-nums text-foreground">
            {sizeEstimate ?? "–"}
          </dd>
        </div>
      </dl>
      {#if removedDuration > 0.05 || hasTrim}
        <p class="text-[11px] text-muted-foreground">
          {#if hasTrim}
            Trimmed to
            <span class="font-mono tabular-nums text-foreground">
              {formatTime(store.trimStart)}–{formatTime(clipEnd)}
            </span>
          {/if}
          {#if removedDuration > 0.05}
            {hasTrim ? "·" : ""} cuts remove
            <span class="font-mono tabular-nums text-foreground">
              {formatTime(removedDuration)}
            </span>
          {/if}
        </p>
      {/if}
    </header>
  </div>

  <!-- Scrollable option list. Format + Quality lead; Frame rate + Speed are
       tucked under Advanced so the common decisions aren't buried in tuning. -->
  <div class="min-h-0 flex-1 overflow-y-auto scrollbar-transparent">
    <div class="flex flex-col gap-4 px-5 py-4">
      {#snippet formatControl()}
        <Segmented
          options={formats.map((f) => ({ value: f.value, label: f.label }))}
          value={store.exportFormat}
          onValueChange={setFormat}
          aria-label="Format"
        />
      {/snippet}
      {@render field("Format", activeFormat?.desc, formatControl)}

      {#snippet qualityControl()}
        <Segmented
          options={qualities.map((q) => ({ value: q.value, label: q.label }))}
          value={store.exportQuality}
          onValueChange={setQuality}
          aria-label="Quality"
        />
      {/snippet}
      {@render field("Quality", activeQuality?.desc, qualityControl)}

      <!-- Advanced tuning. The collapsed row carries its current values so you
           can tell whether opening it is worth it. -->
      <div
        class={cn(
          "flex flex-col rounded-xl border transition-colors",
          advancedOpen ? "border-border/60 bg-card/40" : "border-border/50",
        )}
      >
        <button
          type="button"
          onclick={() => (advancedOpen = !advancedOpen)}
          aria-expanded={advancedOpen}
          aria-controls="export-advanced"
          class="group flex cursor-pointer items-center gap-2 rounded-xl px-3 py-2 text-left transition-colors hover:bg-muted/40 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring"
        >
          <Settings2 class="size-3.5 shrink-0 text-muted-foreground" />
          <span class="shrink-0 text-[11px] font-semibold text-foreground">Advanced</span>
          <span class="ml-auto min-w-0 truncate text-right text-[11px] text-muted-foreground">
            {advancedOpen ? "" : advancedSummary}
          </span>
          <ChevronDown
            class={cn(
              "size-3.5 shrink-0 text-muted-foreground transition-transform duration-200",
              advancedOpen && "rotate-180",
            )}
          />
        </button>
        {#if advancedOpen}
          <div
            id="export-advanced"
            class="flex flex-col gap-4 border-t border-border/50 px-3 py-3"
            transition:slide={{ duration: motionDuration(200), easing: cubicOut }}
          >
            {#if isGif}
              {@render gifSettingsBody()}
            {:else}
              {#if showFps}
                {#snippet fpsControl()}
                  <Segmented
                    options={fpsOptions.map((f) => ({
                      value: String(f.value ?? "original"),
                      label: f.label,
                    }))}
                    value={String(store.exportFps ?? "original")}
                    onValueChange={(v) => setFps(v === "original" ? null : Number(v))}
                    aria-label="Frame rate"
                  />
                {/snippet}
                {@render field("Frame rate", activeFps?.desc, fpsControl)}
              {/if}
              {#snippet speedControl()}
                <Segmented
                  options={speeds.map((sp) => ({ value: sp.value, label: sp.label }))}
                  value={store.exportSpeed}
                  onValueChange={setSpeed}
                  aria-label="Speed"
                />
              {/snippet}
              {@render field("Speed", activeSpeed?.desc, speedControl)}
            {/if}
          </div>
        {/if}
      </div>
      {#if hasCaptions}{@render captionsSection()}{/if}
    </div>
  </div>

  <!-- Sticky footer. -->
  <footer
    class="flex shrink-0 items-center justify-end gap-2 border-t border-border/40 bg-muted/30 px-3 py-2.5"
  >
    <Button variant="ghost" size="xs" onclick={onCancel}>Cancel</Button>
    <Button variant="default" size="xs" class="gap-1.5" onclick={onConfirm}>
      <Upload class="size-3" />
      Export {store.exportFormat.toUpperCase()}
    </Button>
  </footer>
</div>

<style>
  /* Every raw button here is a toggle or action, so show the pointer; disabled ones keep the default. */
  button:not(:disabled) {
    cursor: pointer;
  }
</style>
