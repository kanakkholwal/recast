<script lang="ts">
  import type { EditorStore } from "$lib/stores/editor-store.svelte";
  import { isEditableTarget } from "$lib/dom/editable";
  import { clock } from "$lib/format/time";
  import {
    activePreset as activePresetLabel,
    dbForVolume,
    envelopePath as envelopePathBase,
    FADE_PRESETS,
    volumeZone as classifyVolume,
    type FadePreset,
  } from "./audio-panel.logic";
  import {
    AudioLines,
    AudioWaveform,
    Mic,
    RotateCcw,
    Speaker,
    Waves,
  } from "@recast/icons";
  import { Button } from "@recast/ui/button";
  import { Segmented, SegmentedToggle } from "@recast/ui/segmented";
  import { SliderControl } from "@recast/ui/slider-control";
  import { cubicOut } from "svelte/easing";
  import { scale } from "svelte/transition";
  import { motionDuration } from "$lib/motion.svelte";
  import PanelSection from "./PanelSection.svelte";

  interface Props {
    store: EditorStore;
  }

  let { store }: Props = $props();

  type AudioSettings = EditorStore["audioSettings"];

  function updateAudioSettings(
    updates: Partial<AudioSettings>,
    trackUndo = false,
  ) {
    if (trackUndo) store.pushUndoState();
    store.updateAudioSettings(updates);
  }

  function toggleMute() {
    updateAudioSettings({ muted: !store.audioSettings.muted }, true);
  }
  function resetVolume() {
    updateAudioSettings({ volume: 100 }, true);
  }
  function resetSystemVolume() {
    updateAudioSettings({ systemVolume: 100 }, true);
  }
  function resetMicVolume() {
    updateAudioSettings({ micVolume: 100 }, true);
  }

  // Suppress the M shortcut while typing in an input/contenteditable.
  function handleKey(e: KeyboardEvent) {
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    if (isEditableTarget(e.target)) return;
    if (e.key === "m" || e.key === "M") {
      e.preventDefault();
      toggleMute();
    }
  }

  const volumeZone = $derived(
    classifyVolume(store.audioSettings.muted, store.audioSettings.volume),
  );

  function applyPreset(preset: FadePreset) {
    store.pushUndoState();
    store.updateAudioSettings({ fadeIn: preset.in, fadeOut: preset.out });
  }

  // Matching preset drives the Segmented selection; a custom slider value
  // leaves nothing selected.
  const activePreset = $derived(activePresetLabel(store.audioSettings));
  const fadePresetOptions = $derived(
    FADE_PRESETS.map((p) => ({ value: p.label, label: p.label })),
  );

  // Wrappers: read the reactive store, defer maths to the shared helpers.
  const envelopePath = (fadeIn: number, fadeOut: number): string =>
    envelopePathBase(fadeIn, fadeOut, store.clipDuration || 1);
  const formatClipDuration = (): string => clock(store.clipDuration || 0);
</script>

<!-- `M` toggles master mute. `<svelte:window>` so Svelte rebinds it on each HMR patch. -->
<svelte:window onkeydown={handleKey} />

<div class="flex flex-col gap-4 animate-in fade-in duration-200">
  <PanelSection
    title="Output"
    hint="Master volume affects editor playback and export. Press M to toggle mute."
    flush
  >
    {#snippet action()}
      <div class="flex items-center gap-1">
        <Button
          variant="ghost"
          size="xs"
          class="gap-1 text-muted-foreground hover:text-foreground"
          onclick={resetVolume}
          title="Reset master volume to 100%"
        >
          <RotateCcw size={11} />
          100%
        </Button>
        <SegmentedToggle
          checked={!store.audioSettings.muted}
          offLabel="Muted"
          onLabel="Live"
          size="xs"
          aria-label="Mute (M)"
          onCheckedChange={(next) => {
            store.pushUndoState();
            store.updateAudioSettings({ muted: !next });
          }}
        />
      </div>
    {/snippet}

    <div class="flex flex-col gap-2.5">
      <div
        class="rounded-md border border-border bg-card/60 px-3 py-2.5"
        class:opacity-50={store.audioSettings.muted}
      >
        <div class="flex items-end justify-between gap-2">
          <div>
            <p class="text-[10px] uppercase tracking-wider text-muted-foreground">
              Master gain
            </p>
            <p
              class="font-mono text-2xl font-medium tabular-nums leading-none {volumeZone ===
              'hot'
                ? 'text-destructive'
                : volumeZone === 'boost'
                  ? 'text-warning'
                  : 'text-foreground'}"
            >
              {store.audioSettings.volume}<span
                class="ml-0.5 text-base text-muted-foreground">%</span
              >
            </p>
            <p
              class="mt-0.5 font-mono text-[10px] tabular-nums {volumeZone === 'hot'
                ? 'text-destructive'
                : volumeZone === 'boost'
                  ? 'text-warning'
                  : 'text-muted-foreground'}"
            >
              {dbForVolume(store.audioSettings.volume)}
            </p>
          </div>
          {#if volumeZone === "boost" || volumeZone === "hot"}
            <span
              in:scale={{ start: 0.85, duration: motionDuration(220), easing: cubicOut }}
              class="inline-flex items-center gap-1 rounded-full border px-1.5 py-0.5 font-mono text-[9px] uppercase tracking-wider {volumeZone ===
              'hot'
                ? 'border-destructive/40 bg-destructive/10 text-destructive'
                : 'border-warning/40 bg-warning/10 text-warning'}"
            >
              <Waves size={10} />
              {volumeZone === "hot" ? "Clipping risk" : "Boost"}
            </span>
          {/if}
        </div>

        <div class="relative mt-2 h-1.5 overflow-hidden rounded-full bg-background">
          <div
            class="absolute inset-y-0 left-0 transition-all duration-300 {volumeZone ===
            'hot'
              ? 'bg-destructive'
              : volumeZone === 'boost'
                ? 'bg-warning'
                : volumeZone === 'low'
                  ? 'bg-success/70'
                  : 'bg-success'}"
            style="width: {Math.min(100, (store.audioSettings.volume / 200) * 100)}%"
          ></div>
          <!-- 100% reference tick -->
          <div
            class="absolute inset-y-0 w-px bg-foreground/40"
            style="left: 50%"
            aria-hidden="true"
          ></div>
        </div>
      </div>

      <SliderControl
        label="Output volume"
        value={store.audioSettings.volume}
        min={0}
        max={200}
        step={5}
        unit="%"
        disabled={store.audioSettings.muted}
        onstart={() => store.pushUndoState()}
        onchange={(next) => store.updateAudioSettings({ volume: next })}
        formatValue={(v) => `${v}%`}
      >
        {#snippet icon()}
          <AudioLines size={11} />
        {/snippet}
      </SliderControl>
    </div>
  </PanelSection>

  <PanelSection
    title="Fades"
    hint="Fade the audio in at the start and out at the end."
    flush
    collapsible
  >
    {#snippet action()}
      <span
        class="inline-flex items-center gap-1 text-[10px] text-muted-foreground"
      >
        <AudioWaveform size={10} />
        Envelope
      </span>
    {/snippet}

    <div class="rounded-md border border-border bg-background/60 p-2">
      <svg
        viewBox="0 0 100 24"
        preserveAspectRatio="none"
        class="h-10 w-full"
        aria-hidden="true"
      >
        <path
          d={`${envelopePath(store.audioSettings.fadeIn, store.audioSettings.fadeOut)} L 100 24 L 0 24 Z`}
          class="fill-primary/15"
        />
        <path
          d={envelopePath(store.audioSettings.fadeIn, store.audioSettings.fadeOut)}
          class="stroke-primary/80"
          stroke-width="1.2"
          fill="none"
          vector-effect="non-scaling-stroke"
        />
        <line
          x1="0"
          x2="100"
          y1="2"
          y2="2"
          class="stroke-foreground/15"
          stroke-width="0.5"
          stroke-dasharray="2 2"
        />
      </svg>
      <div
        class="mt-0.5 flex items-center justify-between font-mono text-[9px] tabular-nums text-muted-foreground"
      >
        <span>0:00</span>
        <span>{formatClipDuration()}</span>
      </div>
    </div>

    <div class="mt-2">
      <Segmented
        size="xs"
        aria-label="Fade preset"
        value={activePreset}
        options={fadePresetOptions}
        onValueChange={(v) => {
          const preset = FADE_PRESETS.find((p) => p.label === v);
          if (preset) applyPreset(preset);
        }}
      />
    </div>

    <div class="mt-2.5 space-y-2.5">
      <SliderControl
        label="Fade in"
        value={store.audioSettings.fadeIn}
        min={0}
        max={5}
        step={0.05}
        unit="s"
        onstart={() => store.pushUndoState()}
        onchange={(next) => store.updateAudioSettings({ fadeIn: next })}
        formatValue={(v) => `${v.toFixed(2)}s`}
      />
      <SliderControl
        label="Fade out"
        value={store.audioSettings.fadeOut}
        min={0}
        max={5}
        step={0.05}
        unit="s"
        onstart={() => store.pushUndoState()}
        onchange={(next) => store.updateAudioSettings({ fadeOut: next })}
        formatValue={(v) => `${v.toFixed(2)}s`}
      />
    </div>
  </PanelSection>

  <PanelSection
    title="Loudness"
    hint="Even out overall loudness to about −14 LUFS (the common social target). Applies to the exported file."
    flush
  >
    <div class="flex items-center justify-between">
      <span class="text-[11px] text-foreground">Normalize on export</span>
      <SegmentedToggle
        checked={store.audioSettings.normalizeLoudness}
        size="xs"
        aria-label="Normalize loudness on export"
        onCheckedChange={(next) =>
          updateAudioSettings({ normalizeLoudness: next }, true)}
      />
    </div>
  </PanelSection>

  <!-- Per-track gain: system audio and microphone each get their own level +
       mute, layered on top of the master. The user can keep the system audio
       loud and mute just the mic, or vice versa. Master mute still overrides
       both. -->
  <PanelSection
    title="Sources"
    hint="Per-track gain is layered on the master. Mute one source without touching the other."
    flush
  >
    <div class="flex flex-col gap-3">
      <!-- System audio row -->
      <div
        class="rounded-lg border border-border/60 bg-card/40 p-2.5 shadow-(--shadow-craft-inset)"
      >
        <div class="flex items-center gap-2">
          <span
            class="grid size-7 place-items-center rounded-md bg-muted/60 text-muted-foreground"
            aria-hidden="true"
          >
            <Speaker size={13} />
          </span>
          <div class="min-w-0 flex-1">
            <p class="text-[11px] font-medium text-foreground">System audio</p>
            <p class="truncate font-mono text-[10px] tabular-nums text-muted-foreground">
              {store.audioSettings.systemMuted
                ? "Muted"
                : `${store.audioSettings.systemVolume}%`}
            </p>
          </div>
          <SegmentedToggle
            checked={!store.audioSettings.systemMuted}
            offLabel="Muted"
            onLabel="Live"
            size="xs"
            aria-label="Mute system audio"
            onCheckedChange={(next) => {
              store.pushUndoState();
              store.updateAudioSettings({ systemMuted: !next });
            }}
          />
        </div>
        <div class="mt-2 flex items-center gap-1">
          <SliderControl
            label="Volume"
            value={store.audioSettings.systemVolume}
            min={0}
            max={200}
            step={5}
            unit="%"
            disabled={store.audioSettings.systemMuted || store.audioSettings.muted}
            onstart={() => store.pushUndoState()}
            onchange={(next) => store.updateAudioSettings({ systemVolume: next })}
            formatValue={(v) => `${v}%`}
          >
            {#snippet icon()}
              <Speaker size={11} />
            {/snippet}
          </SliderControl>
          <Button
            variant="ghost"
            size="xs"
            class="h-6 w-6 shrink-0 p-0 text-muted-foreground hover:text-foreground"
            onclick={resetSystemVolume}
            title="Reset system volume to 100%"
            aria-label="Reset system volume"
          >
            <RotateCcw size={11} />
          </Button>
        </div>
      </div>

      <!-- Microphone row -->
      <div
        class="rounded-lg border border-border/60 bg-card/40 p-2.5 shadow-(--shadow-craft-inset)"
      >
        <div class="flex items-center gap-2">
          <span
            class="grid size-7 place-items-center rounded-md bg-muted/60 text-muted-foreground"
            aria-hidden="true"
          >
            <Mic size={13} />
          </span>
          <div class="min-w-0 flex-1">
            <p class="text-[11px] font-medium text-foreground">Microphone</p>
            <p class="truncate font-mono text-[10px] tabular-nums text-muted-foreground">
              {store.audioSettings.micMuted
                ? "Muted"
                : `${store.audioSettings.micVolume}%`}
            </p>
          </div>
          <SegmentedToggle
            checked={!store.audioSettings.micMuted}
            offLabel="Muted"
            onLabel="Live"
            size="xs"
            aria-label="Mute microphone"
            onCheckedChange={(next) => {
              store.pushUndoState();
              store.updateAudioSettings({ micMuted: !next });
            }}
          />
        </div>
        <div class="mt-2 flex items-center gap-1">
          <SliderControl
            label="Volume"
            value={store.audioSettings.micVolume}
            min={0}
            max={200}
            step={5}
            unit="%"
            disabled={store.audioSettings.micMuted || store.audioSettings.muted}
            onstart={() => store.pushUndoState()}
            onchange={(next) => store.updateAudioSettings({ micVolume: next })}
            formatValue={(v) => `${v}%`}
          >
            {#snippet icon()}
              <Mic size={11} />
            {/snippet}
          </SliderControl>
          <Button
            variant="ghost"
            size="xs"
            class="h-6 w-6 shrink-0 p-0 text-muted-foreground hover:text-foreground"
            onclick={resetMicVolume}
            title="Reset microphone volume to 100%"
            aria-label="Reset microphone volume"
          >
            <RotateCcw size={11} />
          </Button>
        </div>
      </div>
    </div>
  </PanelSection>
</div>