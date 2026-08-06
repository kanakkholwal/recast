<script lang="ts">
import type { EditorStore } from "../../stores/editor-store.svelte";
import { isEditableTarget } from "../../lib/dom/editable";
import { clock } from "../../lib/format/time";
import {
	activePreset as activePresetLabel,
	dbForVolume,
	envelopePath as envelopePathBase,
	FADE_PRESETS,
	volumeZone as classifyVolume,
	type FadePreset,
} from "./audio-panel.logic";
import { AudioLines, Mic, MicOff, RotateCcw, Speaker, VolumeOff, Waves } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { Segmented, SegmentedToggle } from "@recast/ui/segmented";
import { SliderControl } from "@recast/ui/slider-control";
import { cubicOut } from "svelte/easing";
import { scale } from "svelte/transition";
import { motionDuration } from "../../lib/motion.svelte";
import PanelSection from "./PanelSection.svelte";
import SettingRow from "./SettingRow.svelte";

interface Props {
	store: EditorStore;
}

let { store }: Props = $props();

type AudioSettings = EditorStore["audioSettings"];

function updateAudioSettings(updates: Partial<AudioSettings>, trackUndo = false) {
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

const volumeZone = $derived(classifyVolume(store.audioSettings.muted, store.audioSettings.volume));

function applyPreset(preset: FadePreset) {
	store.pushUndoState();
	store.updateAudioSettings({ fadeIn: preset.in, fadeOut: preset.out });
}

// Matching preset drives the Segmented selection; a custom slider value
// leaves nothing selected.
const activePreset = $derived(activePresetLabel(store.audioSettings));
const fadePresetOptions = $derived(FADE_PRESETS.map((p) => ({ value: p.label, label: p.label })));

// Wrappers: read the reactive store, defer maths to the shared helpers.
const envelopePath = (fadeIn: number, fadeOut: number): string =>
	envelopePathBase(fadeIn, fadeOut, store.clipDuration || 1);
const formatClipDuration = (): string => clock(store.clipDuration || 0);

// Per-source gain only reaches the export when the source is a SEPARATE track:
// `effective_audio_gain` (commands/editor.rs) ignores system/mic gain for
// muxed `AudioKind::Source` audio. Showing sliders for a track that doesn't
// exist meant setting mic gain to 180% on a recording with no mic.
const hasSystemTrack = $derived(!!store.audioPath);
const hasMicTrack = $derived(!!store.microphonePath);

const zoneText = $derived(
	volumeZone === "hot"
		? "text-destructive"
		: volumeZone === "boost"
			? "text-warning"
			: "text-muted-foreground",
);
</script>

<!-- `M` toggles master mute. Bound here, so it only works while this panel is
     mounted; promoting it to the editor's keymap is a separate change. -->
<svelte:window onkeydown={handleKey} />

<div class="flex flex-col gap-4 animate-in fade-in duration-200">
  <PanelSection
    title="Output"
    hint="Master level for editor playback and export. Press M to toggle mute while this panel is open."
    flush
  >
    {#snippet action()}
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
    {/snippet}

    <div class="flex flex-col gap-2.5">
      <div class="flex items-center gap-1">
        <SliderControl
          class="min-w-0 flex-1"
          label="Master"
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
        <Button
          variant="ghost"
          size="xs"
          class="size-6 shrink-0 p-0 text-muted-foreground hover:text-foreground"
          onclick={resetVolume}
          title="Reset master volume to 100%"
          aria-label="Reset master volume"
        >
          <RotateCcw size={11} />
        </Button>
      </div>

      <!-- dB, plus the boost/clipping warning. The old version put this in a
           hero card with a 0-200 bar that read as a level meter but only ever
           showed the setting the slider already shows. -->
      <div class="flex items-center justify-between gap-2 px-0.5">
        <span class="font-mono text-[10px] tabular-nums {zoneText}">
          {store.audioSettings.muted ? "Muted" : dbForVolume(store.audioSettings.volume)}
        </span>
        {#if !store.audioSettings.muted && (volumeZone === "boost" || volumeZone === "hot")}
          <span
            in:scale={{ start: 0.85, duration: motionDuration(220), easing: cubicOut }}
            class="inline-flex items-center gap-1 rounded-full border px-1.5 py-0.5 text-[10px] font-medium {volumeZone ===
            'hot'
              ? 'border-destructive/40 bg-destructive/10 text-destructive'
              : 'border-warning/40 bg-warning/10 text-warning'}"
          >
            <Waves size={10} />
            {volumeZone === "hot" ? "Clipping risk" : "Boost"}
          </span>
        {/if}
      </div>

      <SettingRow
        label="Normalize on export"
        description="Evens out loudness to about −14 LUFS. Export only."
      >
        {#snippet children(props)}
          <SegmentedToggle
            checked={store.audioSettings.normalizeLoudness}
            size="xs"
            {...props}
            onCheckedChange={(next) =>
              updateAudioSettings({ normalizeLoudness: next }, true)}
          />
        {/snippet}
      </SettingRow>
    </div>
  </PanelSection>

  <!-- Detach-to-timeline is hidden: not production ready. The store side
       (detachRecordingAudio / reattachRecordingAudio / audioDetached) still
       works, so restoring this is uncommenting it plus re-importing AudioWaveform.
  <PanelSection
    title="Timeline audio"
    hint="Detach the recording's audio to trim, move, split, or silence it on its own lane. Once detached it edits independently and no longer follows video cuts."
    flush
  >
    {#if store.audioDetached}
      <div
        class="flex items-center justify-between gap-2 rounded-md border border-lane-audio/40 bg-lane-audio/10 px-2.5 py-2"
      >
        <span class="inline-flex items-center gap-1.5 text-[11px] text-foreground">
          <Mic size={12} class="text-lane-audio" />
          On the Voice lane, edit it there.
        </span>
        <Button variant="outline" size="xs" onclick={() => store.reattachRecordingAudio()}>
          Reattach
        </Button>
      </div>
    {:else}
      <Button
        variant="outline"
        size="sm"
        class="w-full gap-1.5"
        disabled={!store.canDetachAudio}
        onclick={() => store.detachRecordingAudio()}
      >
        <AudioWaveform size={13} /> Detach audio to timeline
      </Button>
      {#if !store.canDetachAudio}
        <p class="mt-1 text-[10px] text-muted-foreground">
          This recording has no separate audio to detach.
        </p>
      {/if}
    {/if}
  </PanelSection>
  -->

  <PanelSection
    title="Fades"
    hint="Fade the audio in at the start and out at the end."
    flush
    collapsible
  >
    {#snippet action()}
      <!-- Dragging either slider off a preset leaves the Segmented with nothing
           selected, which reads as broken unless the state is named. -->
      {#if !activePreset}
        <span class="text-[11px] text-muted-foreground">Custom</span>
      {/if}
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

  <!-- One row per source: level, mute, reset. Both cards were the same markup
       twice, and each showed its value in a caption line the slider already
       renders on the right. -->
  {#snippet sourceRow(kind: "system" | "mic")}
    {@const isSystem = kind === "system"}
    {@const name = isSystem ? "System audio" : "Microphone"}
    {@const level = isSystem
      ? store.audioSettings.systemVolume
      : store.audioSettings.micVolume}
    {@const muted = isSystem
      ? store.audioSettings.systemMuted
      : store.audioSettings.micMuted}
    <div class="flex items-center gap-1">
      <SliderControl
        class="min-w-0 flex-1"
        label={name}
        value={level}
        min={0}
        max={200}
        step={5}
        unit="%"
        disabled={muted || store.audioSettings.muted}
        onstart={() => store.pushUndoState()}
        onchange={(next) =>
          store.updateAudioSettings(
            isSystem ? { systemVolume: next } : { micVolume: next },
          )}
        formatValue={(v) => `${v}%`}
      >
        {#snippet icon()}
          {#if isSystem}
            <Speaker size={11} />
          {:else}
            <Mic size={11} />
          {/if}
        {/snippet}
      </SliderControl>
      <Button
        variant="ghost"
        size="xs"
        class="size-6 shrink-0 p-0 {muted
          ? 'text-destructive hover:text-destructive'
          : 'text-muted-foreground hover:text-foreground'}"
        aria-label="Mute {name}"
        aria-pressed={muted}
        title={muted ? `Unmute ${name}` : `Mute ${name}`}
        onclick={() => {
          store.pushUndoState();
          store.updateAudioSettings(
            isSystem ? { systemMuted: !muted } : { micMuted: !muted },
          );
        }}
      >
        {#if muted}
          {#if isSystem}<VolumeOff size={12} />{:else}<MicOff size={12} />{/if}
        {:else if isSystem}
          <Speaker size={12} />
        {:else}
          <Mic size={12} />
        {/if}
      </Button>
      <Button
        variant="ghost"
        size="xs"
        class="size-6 shrink-0 p-0 text-muted-foreground hover:text-foreground"
        onclick={isSystem ? resetSystemVolume : resetMicVolume}
        title="Reset {name} to 100%"
        aria-label="Reset {name} level"
      >
        <RotateCcw size={11} />
      </Button>
    </div>
  {/snippet}

  {#if hasSystemTrack || hasMicTrack}
    <PanelSection
      title="Sources"
      hint="Per-track level, layered on the master. Mute one source without touching the other."
      flush
    >
      <div class="flex flex-col gap-2">
        {#if hasSystemTrack}{@render sourceRow("system")}{/if}
        {#if hasMicTrack}{@render sourceRow("mic")}{/if}
      </div>
    </PanelSection>
  {/if}
</div>