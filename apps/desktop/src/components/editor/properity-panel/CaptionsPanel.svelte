<script lang="ts">
  import { FONT_WEIGHTS } from "$lib/annotations/palette";
  import { getRecentColors, pushRecentColor } from "$lib/annotations/recent-colors";
  import { formatSize } from "$lib/format/files";
  import { clock } from "$lib/format/time";
  import {
    captionCapabilities,
    deleteCaptionModel,
    downloadCaptionModel,
    exportCaptions,
    hasTranscribableAudio,
    listCaptionModels,
    transcribeProject,
    type CaptionModelInfo,
    type DeviceCapabilities,
  } from "$lib/ipc";
  import { registry } from "$lib/registry";
  import type { CaptionPresetValue } from "$lib/registry/types";
  import {
    captionStyleMatchesPreset,
    downloadProgressPct,
    groupModelsByFamily,
    gpuLabel as gpuLabelOf,
    langLabel,
    pickDefaultModelId,
  } from "./captions-panel.logic";
  import { ensureFontLoaded } from "$lib/fonts/font-options";
  import {
    resolveCaptionAnimation,
    type CaptionAnimation,
  } from "$lib/captions/animation";
  import type { CaptionStyle, EditorStore } from "$lib/stores/editor-store.svelte";
  import { toOutputTimeTranscript } from "$lib/services/export";
  import {
    AlertTriangle,
    AlignCenter,
    AlignLeft,
    AlignRight,
    Check,
    ChevronsUpDown,
    Cpu,
    Download,
    FileDown,
    Loader2,
    Lock,
    MicOff,
    Package,
    Sparkles,
    Trash2,
    Zap
  } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { ColorField } from "@recast/ui/color-field";
  import * as Command from "@recast/ui/command";
  import * as Popover from "@recast/ui/popover";
  import { Segmented, SegmentedToggle } from "@recast/ui/segmented";
  import { SliderControl } from "@recast/ui/slider-control";
  import { toast } from "@recast/ui/sonner";
  import { cn } from "@recast/ui/utils";
  import { onMount } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { fly } from "svelte/transition";
  import FontPicker from "./FontPicker.svelte";
  import PanelSection from "./PanelSection.svelte";

  interface Props {
    store: EditorStore;
  }
  let { store }: Props = $props();

  let models = $state<CaptionModelInfo[]>([]);
  let caps = $state<DeviceCapabilities | null>(null);
  let selectedModelId = $state<string | null>(null);
  let pickerOpen = $state(false);
  let downloadingId = $state<string | null>(null);
  let downloadPct = $state(0);
  let transcribing = $state(false);
  let phase = $state<string>("");
  let error = $state<string | null>(null);

  const selected = $derived(models.find((m) => m.id === selectedModelId) ?? null);
  const usable = $derived(models.filter((m) => m.installed && m.runnable));
  // A recording can have an audio path but no actual audio stream (mic + system
  // audio off), so `hasAudio` is the ffprobe result, not just path existence.
  // `null` = not yet probed → fall back to path presence so the UI doesn't flash
  // the empty state before the probe resolves.
  let audioProbe = $state<boolean | null>(null);
  const pathHasAudio = $derived(!!(store.audioPath || store.microphonePath));
  const hasAudio = $derived(audioProbe ?? pathHasAudio);
  const isDownloadingSelected = $derived(!!selected && downloadingId === selected.id);

  // Re-probe whenever the project's audio sources change (e.g. project reload).
  $effect(() => {
    const paths = [store.audioPath, store.microphonePath];
    if (!paths.some(Boolean)) {
      audioProbe = false;
      return;
    }
    audioProbe = null;
    let cancelled = false;
    hasTranscribableAudio(paths)
      .then((present) => {
        if (!cancelled) audioProbe = present;
      })
      // Don't hard-block on a probe failure. Let the transcribe call be the
      // authority (it reports "no audio" if the extract is truly empty).
      .catch(() => {
        if (!cancelled) audioProbe = true;
      });
    return () => {
      cancelled = true;
    };
  });

  // Group models by family, preserving first-seen order, for the picker.
  const families = $derived(groupModelsByFamily(models));

  const gpuLabel = $derived(gpuLabelOf(caps));

  async function refresh() {
    try {
      models = await listCaptionModels();
      if (!selectedModelId || !models.some((m) => m.id === selectedModelId)) {
        selectedModelId = pickDefaultModelId(models);
      }
    } catch (e) {
      toast.error(`Could not load caption models: ${e}`);
    }
  }

  onMount(() => {
    void refresh();
    captionCapabilities()
      .then((c) => (caps = c))
      .catch(() => {});
  });

  function pick(id: string) {
    selectedModelId = id;
    pickerOpen = false;
  }

  async function handleDownload(id: string) {
    downloadingId = id;
    downloadPct = 0;
    try {
      // Progress is scoped to this download's channel, so no model-id filtering.
      await downloadCaptionModel(id, (p) => {
        downloadPct = downloadProgressPct(p.downloaded, p.total);
      });
      toast.success("Model downloaded");
      await refresh();
    } catch (e) {
      toast.error(`Download failed: ${e}`);
    } finally {
      downloadingId = null;
    }
  }

  async function handleDelete(id: string) {
    try {
      await deleteCaptionModel(id);
      await refresh();
    } catch (e) {
      toast.error(`Could not delete model: ${e}`);
    }
  }

  async function generate() {
    if (!selected || !selected.installed || !selected.runnable || !hasAudio) return;
    transcribing = true;
    phase = "extracting";
    error = null;
    try {
      store.transcript = await transcribeProject({
        audioPath: store.audioPath,
        microphonePath: store.microphonePath,
        modelId: selected.id,
        onPhase: (p) => {
          phase = p.phase;
        },
      });
    } catch (e) {
      error = `${e}`;
      store.transcript = null;
    } finally {
      transcribing = false;
      phase = "";
    }
  }

  async function exportSubs(format: "srt" | "vtt") {
    const t = store.transcript;
    if (!t) return;
    try {
      const { save } = await import("@tauri-apps/plugin-dialog");
      const dest = await save({
        defaultPath: `captions.${format}`,
        filters: [{ name: format.toUpperCase(), extensions: [format] }],
      });
      if (!dest) return;
      // Map onto the output timeline (trim + cuts + speed) so cues line up with
      // the exported video, not the raw recording, using the same warp the export dialog
      // and Cloud track apply.
      await exportCaptions(toOutputTimeTranscript(store, t), format, dest);
      toast.success(`Exported ${format.toUpperCase()}`);
    } catch (e) {
      toast.error(`Export failed: ${e}`);
    }
  }

  const positionOptions = [
    { value: "top", label: "Top" },
    { value: "center", label: "Center" },
    { value: "bottom", label: "Bottom" },
  ];
  const backgroundOptions = [
    { value: "none", label: "None" },
    { value: "soft", label: "Shadow" },
    { value: "box", label: "Box" },
  ];

  const chunkOptions = [
    { value: "line", label: "Line" },
    { value: "phrase", label: "Phrase" },
    { value: "word", label: "Word" },
  ];
  const emphasisOptions = [
    { value: "none", label: "None" },
    { value: "color", label: "Color" },
    { value: "scale", label: "Size" },
  ];
  const entranceOptions = [
    { value: "none", label: "None" },
    { value: "fade", label: "Fade" },
    { value: "pop", label: "Pop" },
    { value: "slide", label: "Slide" },
  ];
  const holdOptions = [
    { value: "hold", label: "Hold" },
    { value: "clear", label: "Clear" },
  ];

  /** Merge a partial animation change into the current (resolved) spec. */
  function updateAnimation(patch: Partial<CaptionAnimation>) {
    const cur = resolveCaptionAnimation(store.captionStyle.animation);
    store.updateCaptionStyle({ animation: { ...cur, ...patch } });
  }

  const CAPTION_SWATCHES = ["#ffffff", "#000000", "#facc15", "#22d3ee", "#f472b6"];

  let recents = $state<string[]>(getRecentColors());
  function rememberColor(c: string) {
    recents = pushRecentColor(c);
  }

  // Caption themes from the asset registry: built-ins first, extension packs
  // appended. Applying one overwrites the style fields but keeps `enabled`.
  const captionPresets = $derived(registry.list("captionPreset"));
  // Preload each preset's font so the picker's live preview chips render in the
  // right typeface (not a fallback) before one is applied.
  $effect(() => {
    for (const p of captionPresets) ensureFontLoaded(p.value.fontFamily, p.value.fontWeight);
  });
  let themeOpen = $state(false);
  function applyPreset(style: Partial<CaptionStyle>) {
    store.updateCaptionStyle(style);
    themeOpen = false;
  }
  // The preset matching the current style exactly (so the picker shows the
  // active theme), or null once the user has tweaked away from any preset.
  const activeTheme = $derived.by(() => {
    const cs = store.captionStyle;
    return captionPresets.find((p) => captionStyleMatchesPreset(cs, p.value)) ?? null;
  });
  const activeThemeLabel = $derived(activeTheme?.label ?? "Custom");
</script>

<div
  class="flex flex-col gap-4"
  in:fly={{ y: 8, duration: 260, delay: 40, easing: cubicOut }}
>
  <PanelSection
    title="Generate captions"
    hint="Transcription runs on your device. No upload, no account."
    flush
    collapsible
    defaultOpen={!store.transcript}
  >
    {#snippet action()}
      {#if caps && hasAudio}
        <span
          class="inline-flex items-center gap-1 rounded-full border border-border/50 bg-card/60 px-1.5 py-0.5 text-[9px] font-medium text-muted-foreground"
          title={caps.gpu.name ?? gpuLabel}
        >
          {#if caps.gpu.available}
            <Zap size={9} class="text-primary" />
          {:else}
            <Cpu size={9} />
          {/if}
          {gpuLabel}
        </span>
      {/if}
    {/snippet}

    {#if caps && !caps.captionsAvailable}
      <!-- On-device captions aren't in this build (Intel Mac, no ONNX Runtime
           for x86_64-apple-darwin). The rest of the editor works normally. -->
      <div
        class="flex flex-col items-center gap-1.5 rounded-lg border border-dashed border-border/60 bg-card/40 px-4 py-6 text-center"
      >
        <Cpu size={20} class="text-muted-foreground" />
        <p class="text-[12px] font-medium text-foreground">Captions not available on Intel Mac</p>
        <p class="max-w-60 text-[10.5px] leading-relaxed text-muted-foreground">
          On-device transcription needs a runtime Apple no longer ships for Intel
          Macs. Everything else in the editor works. Captions run on Apple Silicon,
          Windows, and Linux.
        </p>
      </div>
    {:else if !hasAudio}
      <!-- Nothing to transcribe: a silent recording (no audio stream on the
           video or a separate mic track) can't produce captions. -->
      <div
        class="flex flex-col items-center gap-1.5 rounded-lg border border-dashed border-border/60 bg-card/40 px-4 py-6 text-center"
      >
        <MicOff size={20} class="text-muted-foreground" />
        <p class="text-[12px] font-medium text-foreground">No audio to caption</p>
        <p class="max-w-60 text-[10.5px] leading-relaxed text-muted-foreground">
          This recording has no audio to transcribe. Record with your microphone or
          system audio on to generate captions.
        </p>
      </div>
    {:else}
    <!-- Combobox selector: only the chosen model shows here; the full list
         lives in the popover so the tab stays compact. -->
    <Popover.Root open={pickerOpen} onOpenChange={(v) => (pickerOpen = v)}>
      <Popover.Trigger>
        {#snippet child({ props })}
          <button
            {...props as Record<string, unknown>}
            class="flex w-full items-center gap-2 rounded-lg border border-border/60 bg-card/60 px-2.5 py-2 text-left transition-colors hover:border-border hover:bg-card"
          >
            <span
              class={cn(
                "grid size-7 shrink-0 place-items-center rounded-md",
                selected?.installed && selected?.runnable
                  ? "bg-primary/15 text-primary"
                  : "bg-muted/60 text-muted-foreground",
              )}
            >
              {#if selected && !selected.runnable}
                <Lock size={13} />
              {:else}
                <Package size={13} />
              {/if}
            </span>
            <span class="min-w-0 flex-1">
              <span class="block truncate text-[12px] font-semibold text-foreground">
                {selected?.displayName ?? "Select a model"}
              </span>
              {#if selected}
                <span class="block truncate text-[10px] text-muted-foreground">
                  {selected.family}{#if selected.installed} · Installed{/if}
                </span>
              {/if}
            </span>
            <ChevronsUpDown size={13} class="shrink-0 text-muted-foreground" />
          </button>
        {/snippet}
      </Popover.Trigger>
      <Popover.Content align="start" sideOffset={6} class="w-72 p-0">
        <Command.Root>
          <Command.Input placeholder="Search models…" class="h-9 text-[12px]" />
          <Command.List class="max-h-72 scrollbar-transparent">
            <Command.Empty class="py-6 text-center text-[11px] text-muted-foreground">
              No models found
            </Command.Empty>
            {#each families as fam (fam.name)}
              <Command.Group heading={fam.name}>
                {#each fam.models as m (m.id)}
                  <Command.Item
                    value={`${m.displayName} ${m.family} ${m.engine}`}
                    onSelect={() => pick(m.id)}
                    class="gap-2"
                  >
                    <span class="flex size-4 shrink-0 items-center justify-center">
                      {#if m.id === selectedModelId}<Check size={13} class="text-primary" />{/if}
                    </span>
                    <span class="min-w-0 flex-1 truncate text-[12px]">{m.displayName}</span>
                    {#if !m.runnable}
                      <Lock size={11} class="shrink-0 text-muted-foreground/70" />
                    {:else if m.installed}
                      <Check size={11} class="shrink-0 text-success" />
                    {/if}
                    {#if m.approxSizeBytes}
                      <span class="shrink-0 text-[9.5px] tabular-nums text-muted-foreground">
                        {formatSize(m.approxSizeBytes)}
                      </span>
                    {/if}
                  </Command.Item>
                {/each}
              </Command.Group>
            {/each}
          </Command.List>
        </Command.Root>
      </Popover.Content>
    </Popover.Root>

    <!-- Selected-model detail -->
    {#if selected}
      <div class="mt-2 rounded-lg border border-border/60 bg-card/40 p-2.5">
        <div class="flex flex-wrap items-center gap-1">
          <span
            class="rounded bg-muted/70 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wider text-muted-foreground"
          >
            {selected.engine === "parakeet" ? "Parakeet" : "Whisper"}
          </span>
          <span
            class="rounded bg-muted/70 px-1.5 py-0.5 text-[9px] font-medium text-muted-foreground"
          >
            {langLabel(selected)}
          </span>
          {#if selected.approxSizeBytes}
            <span
              class="rounded bg-muted/70 px-1.5 py-0.5 text-[9px] font-medium tabular-nums text-muted-foreground"
            >
              {formatSize(selected.approxSizeBytes)}
            </span>
          {/if}
          {#if selected.requiresGpu}
            <span class="rounded bg-destructive/10 px-1.5 py-0.5 text-[9px] font-semibold text-destructive">
              Needs GPU
            </span>
          {:else if selected.prefersGpu}
            <span class="rounded bg-warning/10 px-1.5 py-0.5 text-[9px] font-medium text-warning">
              Faster with GPU
            </span>
          {/if}
          {#if selected.minRamBytes}
            <span
              class="rounded bg-muted/70 px-1.5 py-0.5 text-[9px] font-medium tabular-nums text-muted-foreground"
            >
              ≥ {formatSize(selected.minRamBytes)} RAM
            </span>
          {/if}
        </div>

        {#if selected.warning}
          <p
            class={cn(
              "mt-2 flex items-start gap-1.5 text-[10px] leading-tight",
              selected.runnable ? "text-warning" : "text-muted-foreground",
            )}
          >
            <AlertTriangle size={11} class="mt-px shrink-0" />
            <span>{selected.warning}</span>
          </p>
        {/if}

        <div class="mt-2.5">
          {#if !selected.runnable}
            <p class="flex items-center gap-1.5 text-[11px] font-medium text-muted-foreground">
              <Lock size={12} /> Unavailable on this device
            </p>
          {:else if isDownloadingSelected}
            <div class="flex items-center gap-2">
              <div class="relative h-1.5 flex-1 overflow-hidden rounded-full bg-muted">
                <div
                  class="absolute inset-y-0 left-0 bg-primary transition-all"
                  style="width: {downloadPct}%"
                ></div>
              </div>
              <span class="text-[10px] tabular-nums text-muted-foreground">{downloadPct}%</span>
            </div>
          {:else if selected.installed}
            <div class="flex items-center justify-between">
              <span class="flex items-center gap-1.5 text-[11px] font-medium text-success">
                <Check size={13} /> Installed
              </span>
              <Button
                variant="ghost"
                size="xs"
                class="h-7 gap-1 text-[11px] text-muted-foreground hover:text-destructive"
                onclick={() => handleDelete(selected.id)}
              >
                <Trash2 size={12} /> Remove
              </Button>
            </div>
          {:else if selected.downloadable}
            <Button
              variant="secondary"
              size="sm"
              class="w-full gap-1.5"
              disabled={!!downloadingId}
              onclick={() => handleDownload(selected.id)}
            >
              <Download size={13} /> Download model
            </Button>
          {:else}
            <p class="text-[11px] text-muted-foreground">Coming soon.</p>
          {/if}
        </div>
      </div>
    {/if}

    <!-- Generate lives in the same section as the model picker. -->
    <div class="mt-3 border-t border-border/50 pt-3">
      <Button
        variant="default"
        size="sm"
        class="w-full gap-1.5"
        disabled={!selected?.installed || !selected?.runnable || transcribing}
        onclick={generate}
      >
        {#if transcribing}
          <Loader2 size={14} class="animate-spin" />
          {phase === "extracting" ? "Reading audio…" : "Transcribing…"}
        {:else}
          <Sparkles size={14} />
          {store.transcript ? "Regenerate captions" : "Generate captions"}
        {/if}
      </Button>

      {#if usable.length === 0}
        <p class="mt-2 text-[10.5px] text-muted-foreground">
          Download a model your device can run to enable captioning.
        </p>
      {/if}

      {#if error}
        <div
          class="mt-2 flex items-start gap-1.5 rounded-md border border-warning/40 bg-warning/10 px-2 py-1.5 text-[10.5px] text-warning"
        >
          <AlertTriangle size={12} class="mt-px shrink-0" />
          <span class="min-w-0">{error}</span>
        </div>
      {/if}
    </div>
    {/if}
  </PanelSection>

  {#if store.transcript && store.transcript.segments.length > 0}
    {@const cs = store.captionStyle}
    <PanelSection title="Style" hint="How captions look over the preview and in burned-in exports." flush>
      {#snippet action()}
        <SegmentedToggle
          checked={cs.enabled}
          offLabel="Hidden"
          onLabel="Shown"
          size="xs"
          aria-label="Show captions in preview"
          onCheckedChange={(next) => store.updateCaptionStyle({ enabled: next })}
        />
      {/snippet}

      <div class="flex flex-col gap-3" class:opacity-50={!cs.enabled}>
        {#if captionPresets.length > 0}
          <div class="flex items-center justify-between gap-2">
            <span class="text-[10px] font-medium uppercase tracking-wider text-muted-foreground">
              Theme
            </span>
            {#snippet themeSwatch(v: CaptionPresetValue)}
              {@const accent =
                v.animation && v.animation.emphasis !== "none" ? v.animation.emphasisColor : null}
              <span
                class="grid h-6 w-11 shrink-0 place-items-center overflow-hidden rounded bg-[#0b0b12]"
                aria-hidden="true"
              >
                <span
                  class="leading-none"
                  style="font-family: {v.fontFamily}; font-weight: {v.fontWeight}; font-size: 14px;
                    text-transform: {v.uppercase ? 'uppercase' : 'none'}; color: {v.color};
                    {v.outlineWidth > 0
                    ? `-webkit-text-stroke: ${(Math.min(v.outlineWidth, 8) / 100) * 14}px ${v.outlineColor}; paint-order: stroke fill;`
                    : ''}"
                >A<span style={accent ? `color: ${accent};` : ""}>a</span></span>
              </span>
            {/snippet}

            <Popover.Root open={themeOpen} onOpenChange={(v) => (themeOpen = v)}>
              <Popover.Trigger>
                {#snippet child({ props })}
                  <button
                    {...props}
                    class="flex h-7 w-36 items-center gap-1.5 rounded-md border border-border/60 bg-card/60 pl-1 pr-2 text-left text-[11px] transition-colors hover:border-border hover:bg-card"
                  >
                    {#if activeTheme}
                      {@render themeSwatch(activeTheme.value)}
                    {/if}
                    <span class="min-w-0 flex-1 truncate">{activeThemeLabel}</span>
                    <ChevronsUpDown size={12} class="shrink-0 text-muted-foreground" />
                  </button>
                {/snippet}
              </Popover.Trigger>
              <Popover.Content align="end" sideOffset={6} class="w-64 p-0">
                <Command.Root>
                  <Command.Input placeholder="Search themes…" class="h-9 text-[12px]" />
                  <Command.List class="max-h-80 scrollbar-transparent pt-2">
                    <Command.Empty class="py-6 text-center text-[11px] text-muted-foreground">
                      No themes found
                    </Command.Empty>
                    {#each captionPresets as preset (preset.id)}
                      <Command.Item
                        value={`${preset.label} ${preset.description ?? ""}`}
                        onSelect={() => applyPreset(preset.value)}
                        class="gap-2"
                      >
                        <span class="flex size-4 shrink-0 items-center justify-center">
                          {#if activeTheme?.id === preset.id}<Check size={13} class="text-primary" />{/if}
                        </span>
                        {@render themeSwatch(preset.value)}
                        <span class="min-w-0 flex-1 truncate text-[12px]">{preset.label}</span>
                        {#if preset.description}
                          <span class="shrink-0 text-[10px] text-muted-foreground">{preset.description}</span>
                        {/if}
                      </Command.Item>
                    {/each}
                  </Command.List>
                </Command.Root>
              </Popover.Content>
            </Popover.Root>
          </div>
        {/if}

        <div class="flex items-center justify-between gap-2">
          <span class="text-[10px] text-muted-foreground">Font</span>
          <FontPicker
            value={cs.fontFamily}
            weight={cs.fontWeight}
            onChange={(v) => store.updateCaptionStyle({ fontFamily: v })}
          />
        </div>

        <div class="flex items-center justify-between gap-2">
          <span class="text-[10px] text-muted-foreground">Weight</span>
          <Segmented
            size="xs"
            fill={false}
            aria-label="Caption font weight"
            value={String(cs.fontWeight)}
            options={FONT_WEIGHTS.map((w) => ({
              value: String(w.value),
              label: w.label,
              title: w.title,
            }))}
            onValueChange={(v) => store.updateCaptionStyle({ fontWeight: Number(v) })}
          />
        </div>

        <SliderControl
          label="Font size"
          value={cs.fontSizePct}
          min={2}
          max={10}
          step={0.5}
          unit="%"
          onchange={(next) => store.updateCaptionStyle({ fontSizePct: next })}
          formatValue={(v) => `${v}%`}
        />

        <div class="flex items-center justify-between gap-2">
          <span class="text-[10px] text-muted-foreground">Uppercase</span>
          <SegmentedToggle
            checked={cs.uppercase}
            offLabel="Off"
            onLabel="On"
            size="xs"
            aria-label="Uppercase captions"
            onCheckedChange={(next) => store.updateCaptionStyle({ uppercase: next })}
          />
        </div>

        <div class="flex items-center justify-between gap-2">
          <span class="text-[10px] text-muted-foreground">Position</span>
          <Segmented
            size="xs"
            fill={false}
            aria-label="Caption position"
            value={cs.position}
            options={positionOptions}
            onValueChange={(v) =>
              store.updateCaptionStyle({ position: v as "top" | "center" | "bottom" })}
          />
        </div>

        <div class="flex items-center justify-between gap-2">
          <span class="text-[10px] text-muted-foreground">Align</span>
          {#snippet alignLeftIcon()}<AlignLeft size={12} />{/snippet}
          {#snippet alignCenterIcon()}<AlignCenter size={12} />{/snippet}
          {#snippet alignRightIcon()}<AlignRight size={12} />{/snippet}
          <Segmented
            size="xs"
            fill={false}
            aria-label="Caption alignment"
            value={cs.align}
            options={[
              { value: "left", icon: alignLeftIcon, title: "Left" },
              { value: "center", icon: alignCenterIcon, title: "Center" },
              { value: "right", icon: alignRightIcon, title: "Right" },
            ]}
            onValueChange={(v) =>
              store.updateCaptionStyle({ align: v as "left" | "center" | "right" })}
          />
        </div>

        {#if cs.position !== "center"}
          <SliderControl
            label="Offset"
            value={cs.offsetPct}
            min={-20}
            max={40}
            step={0.5}
            unit="%"
            onchange={(next) => store.updateCaptionStyle({ offsetPct: next })}
            formatValue={(v) => `${v}%`}
          />
        {/if}

        <ColorField
          label="Color"
          value={cs.color}
          swatches={CAPTION_SWATCHES}
          {recents}
          oncommit={(c) => {
            store.updateCaptionStyle({ color: c });
            rememberColor(c);
          }}
        />

        <SliderControl
          label="Max lines"
          value={cs.maxLines}
          min={1}
          max={4}
          step={1}
          unit=""
          onchange={(next) => store.updateCaptionStyle({ maxLines: next })}
          formatValue={(v) => `${v}`}
        />
      </div>
    </PanelSection>

    <PanelSection
      title="Animation"
      hint="Word-by-word reveal and highlight, synced to speech. Needs word timing, so pick a Parakeet model for the tightest sync."
      flush
      collapsible
      defaultOpen={false}
    >
      {@const ca = resolveCaptionAnimation(cs.animation)}
      <div class="flex flex-col gap-3" class:opacity-50={!cs.enabled}>
        <div class="flex items-center justify-between gap-2">
          <span class="text-[10px] text-muted-foreground">Show</span>
          <Segmented
            size="xs"
            fill={false}
            aria-label="Words shown at once"
            value={ca.chunk}
            options={chunkOptions}
            onValueChange={(v) => updateAnimation({ chunk: v as CaptionAnimation["chunk"] })}
          />
        </div>

        {#if ca.chunk === "phrase"}
          <SliderControl
            label="Words per chunk"
            value={ca.chunkSize}
            min={1}
            max={8}
            step={1}
            unit=""
            onchange={(next) => updateAnimation({ chunkSize: next })}
            formatValue={(v) => `${v}`}
          />
        {/if}

        <div class="flex items-center justify-between gap-2">
          <span class="text-[10px] text-muted-foreground">Active word</span>
          <Segmented
            size="xs"
            fill={false}
            aria-label="Active word emphasis"
            value={ca.emphasis}
            options={emphasisOptions}
            onValueChange={(v) => updateAnimation({ emphasis: v as CaptionAnimation["emphasis"] })}
          />
        </div>

        {#if ca.emphasis === "color"}
          <ColorField
            label="Highlight color"
            value={ca.emphasisColor}
            swatches={CAPTION_SWATCHES}
            {recents}
            oncommit={(c) => {
              updateAnimation({ emphasisColor: c });
              rememberColor(c);
            }}
          />
        {/if}

        <div class="flex items-center justify-between gap-2">
          <span class="text-[10px] text-muted-foreground">Entrance</span>
          <Segmented
            size="xs"
            fill={false}
            aria-label="Entrance animation"
            value={ca.entrance}
            options={entranceOptions}
            onValueChange={(v) => updateAnimation({ entrance: v as CaptionAnimation["entrance"] })}
          />
        </div>

        {#if ca.entrance !== "none"}
          <SliderControl
            label="Entrance speed"
            value={ca.entranceMs}
            min={80}
            max={600}
            step={20}
            unit="ms"
            onchange={(next) => updateAnimation({ entranceMs: next })}
            formatValue={(v) => `${v}ms`}
          />
        {/if}

        {#if ca.emphasis !== "none"}
          <div class="flex items-center justify-between gap-2">
            <span class="text-[10px] text-muted-foreground">In pauses</span>
            <Segmented
              size="xs"
              fill={false}
              aria-label="Highlight behavior during pauses"
              value={ca.holdGaps ? "hold" : "clear"}
              options={holdOptions}
              onValueChange={(v) => updateAnimation({ holdGaps: v === "hold" })}
            />
          </div>
        {/if}
      </div>
    </PanelSection>

    <PanelSection
      title="Background & outline"
      hint="Keep captions legible over any footage with a backing box, a stroke, and finer spacing."
      flush
      collapsible
      defaultOpen={false}
    >
      <div class="flex flex-col gap-3" class:opacity-50={!cs.enabled}>
        <div class="flex items-center justify-between gap-2">
          <span class="text-[10px] text-muted-foreground">Background</span>
          <Segmented
            size="xs"
            fill={false}
            aria-label="Caption background"
            value={cs.background}
            options={backgroundOptions}
            onValueChange={(v) =>
              store.updateCaptionStyle({ background: v as "none" | "soft" | "box" })}
          />
        </div>

        {#if cs.background === "box"}
          <ColorField
            label="Box color"
            value={cs.backgroundColor}
            swatches={CAPTION_SWATCHES}
            {recents}
            oncommit={(c) => {
              store.updateCaptionStyle({ backgroundColor: c });
              rememberColor(c);
            }}
          />

          <SliderControl
            label="Box opacity"
            value={cs.backgroundOpacity}
            min={0}
            max={100}
            step={5}
            unit="%"
            onchange={(next) => store.updateCaptionStyle({ backgroundOpacity: next })}
            formatValue={(v) => `${v}%`}
          />
        {/if}

        <SliderControl
          label="Outline"
          value={cs.outlineWidth}
          min={0}
          max={10}
          step={0.5}
          unit=""
          onchange={(next) => store.updateCaptionStyle({ outlineWidth: next })}
          formatValue={(v) => (v === 0 ? "None" : `${v}`)}
        />

        {#if cs.outlineWidth > 0}
          <ColorField
            label="Outline color"
            value={cs.outlineColor}
            swatches={CAPTION_SWATCHES}
            {recents}
            oncommit={(c) => {
              store.updateCaptionStyle({ outlineColor: c });
              rememberColor(c);
            }}
          />
        {/if}

        <SliderControl
          label="Letter spacing"
          value={cs.letterSpacing}
          min={-0.05}
          max={0.3}
          step={0.01}
          unit="em"
          onchange={(next) => store.updateCaptionStyle({ letterSpacing: next })}
          formatValue={(v) => `${v.toFixed(2)}em`}
        />
      </div>
    </PanelSection>

    <PanelSection title="Transcript" hint="Click a line to jump the playhead there." flush>
      {#snippet action()}
        <div class="flex items-center gap-1">
          <Button variant="ghost" size="xs" class="h-6 gap-1 text-[10px]" onclick={() => exportSubs("srt")}>
            <FileDown size={11} /> SRT
          </Button>
          <Button variant="ghost" size="xs" class="h-6 gap-1 text-[10px]" onclick={() => exportSubs("vtt")}>
            <FileDown size={11} /> VTT
          </Button>
        </div>
      {/snippet}

      <div class="flex flex-col gap-0.5">
        {#each store.transcript.segments as seg (seg.id)}
          <button
            type="button"
            class="group flex items-start gap-2 rounded-md px-1.5 py-1 text-left transition-colors hover:bg-muted/60"
            onclick={() => store.seek(seg.start)}
          >
            <span
              class="shrink-0 pt-px font-mono text-[9.5px] tabular-nums text-muted-foreground/70 group-hover:text-foreground"
            >
              {clock(seg.start)}
            </span>
            <span class="min-w-0 text-[11.5px] leading-snug text-foreground">{seg.text}</span>
          </button>
        {/each}
      </div>
    </PanelSection>
  {/if}
</div>
