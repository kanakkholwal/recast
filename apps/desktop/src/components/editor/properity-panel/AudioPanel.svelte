<script lang="ts">
  import type { EditorStore } from "$lib/stores/editor-store.svelte";
  import {
    AudioLines,
    AudioWaveform,
    Mic,
    RotateCcw,
    Speaker,
    Volume2,
    VolumeX,
    Waves,
  } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { cn } from "@recast/ui/utils";
  import { onDestroy, onMount } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { fly, scale } from "svelte/transition";
  import { SliderControl } from "@recast/ui/slider-control";
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

  // M keyboard shortcut. Suppressed inside text inputs and contenteditable
  // (e.g. text annotations) so it doesn't fire while the user is typing.
  function isEditableTarget(target: EventTarget | null): boolean {
    if (!(target instanceof HTMLElement)) return false;
    if (target.isContentEditable) return true;
    const tag = target.tagName;
    return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT";
  }
  function handleKey(e: KeyboardEvent) {
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    if (isEditableTarget(e.target)) return;
    if (e.key === "m" || e.key === "M") {
      e.preventDefault();
      toggleMute();
    }
  }
  onMount(() => window.addEventListener("keydown", handleKey));
  onDestroy(() => window.removeEventListener("keydown", handleKey));

  // Volume zones for the slider readout. Above 100% the export pipeline
  // applies straight gain, which can clip — surface that as a warning.
  type Zone = "muted" | "low" | "nominal" | "boost" | "hot";
  const volumeZone = $derived.by<Zone>(() => {
    if (store.audioSettings.muted) return "muted";
    const v = store.audioSettings.volume;
    if (v <= 0) return "muted";
    if (v < 70) return "low";
    if (v <= 105) return "nominal";
    if (v <= 150) return "boost";
    return "hot";
  });

  // dB-ish display ("0 dB" at 100%, calibrated as 20·log10(volume/100)). Not
  // strictly the same as the Rust ffmpeg `volume=` filter (which is a
  // multiplier on the linear sample value) but matches user intuition.
  function dbForVolume(v: number): string {
    if (v <= 0) return "−∞ dB";
    const db = 20 * Math.log10(v / 100);
    if (Math.abs(db) < 0.05) return "0.0 dB";
    return `${db > 0 ? "+" : ""}${db.toFixed(1)} dB`;
  }

  const FADE_PRESETS: Array<{ label: string; in: number; out: number }> = [
    { label: "None", in: 0, out: 0 },
    { label: "Subtle", in: 0.25, out: 0.25 },
    { label: "Smooth", in: 0.5, out: 1.0 },
    { label: "Cinematic", in: 1.0, out: 2.0 },
  ];

  function applyPreset(preset: (typeof FADE_PRESETS)[number]) {
    store.pushUndoState();
    store.updateAudioSettings({ fadeIn: preset.in, fadeOut: preset.out });
  }
  function isPresetActive(preset: (typeof FADE_PRESETS)[number]): boolean {
    const a = store.audioSettings;
    return (
      Math.abs(a.fadeIn - preset.in) < 0.01 &&
      Math.abs(a.fadeOut - preset.out) < 0.01
    );
  }

  // SVG path for a tiny gain envelope visualization. Mirrors the FFmpeg
  // afade behaviour: linear ramp 0→1 over fadeIn at the head, hold at 1,
  // then linear ramp 1→0 over fadeOut at the tail.
  function envelopePath(fadeIn: number, fadeOut: number): string {
    const W = 100;
    const H = 24;
    const totalSecs = Math.max(0.01, store.clipDuration || 1);
    const fi = Math.max(0, Math.min(fadeIn, totalSecs * 0.5));
    const fo = Math.max(0, Math.min(fadeOut, totalSecs * 0.5));
    const xIn = (fi / totalSecs) * W;
    const xOut = W - (fo / totalSecs) * W;
    const yTop = 2;
    const yBottom = H - 2;
    return `M 0 ${yBottom} L ${xIn.toFixed(2)} ${yTop} L ${xOut.toFixed(2)} ${yTop} L ${W} ${yBottom}`;
  }

  function formatClipDuration(): string {
    const t = Math.max(0, store.clipDuration || 0);
    const m = Math.floor(t / 60);
    const s = Math.floor(t % 60);
    return `${m}:${s.toString().padStart(2, "0")}`;
  }

  // Derived "tracks" summary. Today the export pipeline mixes system audio
  // and microphone behind one volume; this section labels the channels for
  // discoverability and is structured so per-track gain (Phase 2 audio
  // work) can land here without redoing the layout.
  type Track = {
    id: "output" | "system" | "mic";
    label: string;
    icon: typeof Speaker;
    state: "live" | "muted" | "absent";
    description: string;
  };

  const tracks = $derived.by<Track[]>(() => {
    const muted = store.audioSettings.muted || store.audioSettings.volume <= 0;
    const out: Track[] = [
      {
        id: "output",
        label: "Master output",
        icon: AudioLines,
        state: muted ? "muted" : "live",
        description: muted
          ? "Silenced in playback and export."
          : `${store.audioSettings.volume}% · ${dbForVolume(store.audioSettings.volume)}`,
      },
      {
        id: "system",
        label: "System audio",
        icon: Speaker,
        state: muted ? "muted" : "live",
        description: "Captured from the recording session.",
      },
      {
        id: "mic",
        label: "Microphone",
        icon: Mic,
        state: muted ? "muted" : "live",
        description:
          "Mixed into the master at the same gain. (Per-track gain coming next.)",
      },
    ];
    return out;
  });
</script>

<div class="flex flex-col gap-4" in:fly={{ y: 8, duration: 260, delay: 40, easing: cubicOut }}>
  <!-- Master gain readout + mute -->
  <PanelSection
    title="Master"
    hint="Volume affects editor playback and export. Fades are applied during export. Press M to toggle mute."
    flush
  >
    {#snippet action()}
      <div class="flex items-center gap-1">
        <Button
          variant="ghost"
          size="xs"
          class="gap-1 text-muted-foreground hover:text-foreground"
          onclick={resetVolume}
          title="Reset volume to 100%"
        >
          <RotateCcw size={11} />
          100%
        </Button>
        <Button
          variant={store.audioSettings.muted
            ? "destructive_soft"
            : "default_soft"}
          size="xs"
          class="gap-1.5"
          onclick={toggleMute}
          aria-pressed={store.audioSettings.muted}
          title="Toggle mute (M)"
        >
          {#if store.audioSettings.muted}
            <VolumeX size={11} />
            Muted
          {:else}
            <Volume2 size={11} />
            Live
          {/if}
        </Button>
      </div>
    {/snippet}

    <!-- Big readout: master gain + dB -->
    <div
      class="rounded-md border border-border bg-card/60 px-3 py-2.5"
      class:opacity-50={store.audioSettings.muted}
    >
      <div class="flex items-end justify-between gap-2">
        <div>
          <p
            class="text-[10px] uppercase tracking-wider text-muted-foreground"
          >
            Output gain
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
            class="mt-0.5 font-mono text-[10px] tabular-nums {volumeZone ===
            'hot'
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
            in:scale={{ start: 0.85, duration: 220, easing: cubicOut }}
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

      <!-- Linear gain bar with 100% reference mark -->
      <div
        class="relative mt-2 h-1.5 overflow-hidden rounded-full bg-background"
      >
        <div
          class="absolute inset-y-0 left-0 transition-all duration-300 {volumeZone === 'hot'
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
  </PanelSection>

  <PanelSection
    title="Tracks"
    hint="System audio and microphone share the master gain today. Per-track levels land in the next audio pass."
    flush
  >
    <ul class="flex flex-col gap-1">
      {#each tracks as track, i (track.id)}
        {@const Icon = track.icon}
        <li
          in:fly={{ y: 6, duration: 240, delay: 140 + i * 50, easing: cubicOut }}
          class={cn(
            "flex items-center gap-2 rounded-md border border-border/60 bg-background/40 px-2.5 py-1.5 shadow-(--shadow-craft-inset) transition-opacity",
            track.state === "muted" && "opacity-60",
          )}
        >
          <span
            class={cn(
              "flex size-7 items-center justify-center rounded border",
              track.state === "muted"
                ? "border-border bg-muted text-muted-foreground"
                : "border-primary/30 bg-primary/10 text-primary",
            )}
          >
            <Icon size={12} />
          </span>
          <div class="flex-1 min-w-0">
            <p class="text-[11px] font-medium text-foreground">
              {track.label}
            </p>
            <p class="truncate text-[10px] text-muted-foreground">
              {track.description}
            </p>
          </div>
          <span
            class="inline-flex items-center gap-1 font-mono text-[9px] uppercase tracking-wider"
          >
            <span
              class={cn(
                "size-1.5 rounded-full",
                track.state === "live"
                  ? "bg-success"
                  : "bg-muted-foreground",
              )}
            ></span>
            {track.state === "live" ? "Live" : "Muted"}
          </span>
        </li>
      {/each}
    </ul>
  </PanelSection>

  <PanelSection
    title="Mix"
    hint="Mute preserves the chosen volume so the toggle restores the previous level."
  >
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
  </PanelSection>

  <PanelSection
    title="Fades"
    hint="Fades are export-side only — playback stays responsive while you edit."
    flush
  >
    {#snippet action()}
      <span class="inline-flex items-center gap-1 text-[10px] text-muted-foreground">
        <AudioWaveform size={10} />
        Envelope
      </span>
    {/snippet}

    <!-- Envelope visualization -->
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

    <!-- Presets -->
    <div class="mt-2 flex items-center gap-1">
      {#each FADE_PRESETS as preset, i (preset.label)}
        {@const isActive = isPresetActive(preset)}
        <span
          class="flex flex-1"
          in:scale={{ start: 0.92, duration: 220, delay: 280 + i * 35, easing: cubicOut }}
        >
          <Button
            variant="raw"
            size="xs"
            aria-pressed={isActive}
            onclick={() => applyPreset(preset)}
            class={cn(
              "flex-1 gap-1 rounded-md border text-[10px] transition-colors",
              isActive
                ? "border-primary/60 bg-primary/10 text-primary ring-1 ring-primary/30"
                : "border-border/60 bg-background text-muted-foreground hover:border-border hover:text-foreground",
            )}
            title="Set fade in to {preset.in.toFixed(2)}s and fade out to {preset.out.toFixed(2)}s"
          >
            {preset.label}
          </Button>
        </span>
      {/each}
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
</div>
