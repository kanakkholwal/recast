<script lang="ts" module>
type StyleView = "style" | "motion" | "text";
// Survives the panel unmounting when you switch rail tabs, so coming back to
// Captions puts you where you left off.
let lastView: StyleView = "style";
</script>

<script lang="ts">
import { type CaptionAnimation, resolveCaptionAnimation } from "@recast/captions";
import {
	AiWand,
	AlertTriangle,
	AlignCenter,
	AlignLeft,
	AlignRight,
	Check,
	ChevronsUpDown,
	Cpu,
	Download,
	FileDown,
	Info,
	LoaderCircle,
	Lock,
	MicOff,
	Package,
	Search,
	Trash2,
	X,
	Zap,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import { ColorField } from "@recast/ui/color-field";
import * as Command from "@recast/ui/command";
import * as Popover from "@recast/ui/popover";
import { Segmented, SegmentedToggle } from "@recast/ui/segmented";
import SliderRow from "./SliderRow.svelte";
import { toast } from "@recast/ui/sonner";
import { Switch } from "@recast/ui/switch";
import * as Tabs from "@recast/ui/tabs";
import { cn } from "@recast/ui/utils";
import { onMount } from "svelte";
import { FONT_WEIGHTS } from "../../lib/annotations/palette";
import { getRecentColors, pushRecentColor } from "../../lib/annotations/recent-colors";
import { ensureFontLoaded } from "../../lib/fonts/font-options";
import { formatSize } from "../../lib/format/files";
import { clock } from "../../lib/format/time";
import {
	type CaptionModelInfo,
	type DeviceCapabilities,
	getEditorServices,
	TRANSCRIBE_CANCELLED,
} from "../../lib/editor/services";
import { registry } from "../../lib/registry";
import type { CaptionPresetValue } from "../../lib/registry/types";
import { toOutputTimeTranscript } from "../../lib/services/export";
import type { CaptionStyle, EditorStore } from "../../stores/editor-store.svelte";
import { experimentalStore } from "../../stores/experimental.svelte";
import CaptionThemePicker from "./CaptionThemePicker.svelte";
import {
	captionStyleMatchesPreset,
	downloadProgressPct,
	elapsedLabel,
	filterSegments,
	gpuLabel as gpuLabelOf,
	groupModelsByFamily,
	langLabel,
	pickDefaultModelId,
} from "./captions-panel.logic";
import FontPicker from "./FontPicker.svelte";
import PanelSection from "./PanelSection.svelte";
import PropRow from "./PropRow.svelte";
import PropSelect from "./PropSelect.svelte";
import SettingRow from "./SettingRow.svelte";

interface Props {
	store: EditorStore;
}
let { store }: Props = $props();

// Absent on hosts with no on-device ASR (web): the whole generate surface —
// model picker, download, probe — hides and import stays the only way in.
const services = getEditorServices();
const asr = services.transcription;
const captionFiles = services.captionFiles;

let view = $state<StyleView>(lastView);
function setView(next: StyleView) {
	view = next;
	lastView = next;
}

let models = $state<CaptionModelInfo[]>([]);
let caps = $state<DeviceCapabilities | null>(null);
let selectedModelId = $state<string | null>(null);
let pickerOpen = $state(false);
let downloadingId = $state<string | null>(null);
let downloadPct = $state(0);
let transcribing = $state(false);
let cancelling = $state(false);
let phase = $state<string>("");
let error = $state<string | null>(null);
let transcriptQuery = $state("");

// No percentage to report: the Rust side runs inference as one blocking call and
// only emits coarse phases. Elapsed time is the honest substitute for a spinner
// that could sit there for minutes.
let startedAt = 0;
let elapsedMs = $state(0);
$effect(() => {
	if (!transcribing) return;
	const id = setInterval(() => {
		elapsedMs = Date.now() - startedAt;
	}, 1000);
	return () => clearInterval(id);
});

const selected = $derived(models.find((m) => m.id === selectedModelId) ?? null);
const usable = $derived(models.filter((m) => m.installed && m.runnable && m.runtimeAvailable));
// Remote endpoints transcribe over HTTP, so they work even where the on-device
// engine isn't in the build (Intel Mac). Their presence lets us offer captions
// there instead of a dead end.
const hasRemoteModels = $derived(models.some((m) => m.source === "remote"));
// A recording can have an audio path but no actual audio stream (mic + system
// audio off), so `hasAudio` is the ffprobe result, not just path existence.
// `null` = not yet probed → fall back to path presence so the UI doesn't flash
// the empty state before the probe resolves.
let audioProbe = $state<boolean | null>(null);
const pathHasAudio = $derived(!!(store.audioPath || store.microphonePath));
const hasAudio = $derived(audioProbe ?? pathHasAudio);
const isDownloadingSelected = $derived(!!selected && downloadingId === selected.id);

// Secondary model facts as one sentence instead of five more pills.
const capabilityLine = $derived.by(() => {
	if (!selected) return "";
	const bits: string[] = [];
	if (selected.minRamBytes) bits.push(`Needs ${formatSize(selected.minRamBytes)} RAM`);
	if (selected.capabilities.streaming) bits.push("streams as it runs");
	if (selected.capabilities.translate) bits.push("can translate");
	if (selected.capabilities.langDetect) bits.push("detects the language");
	return bits.join(" · ");
});

// Re-probe whenever the project's audio sources change (e.g. project reload).
$effect(() => {
	const paths = [store.audioPath, store.microphonePath];
	if (!asr || !paths.some(Boolean)) {
		audioProbe = false;
		return;
	}
	audioProbe = null;
	let cancelled = false;
	asr.hasTranscribableAudio(paths)
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
	if (!asr) return;
	try {
		const list = await asr.listModels();
		// Remote endpoints are an experimental surface; hide them (and any models
		// they contribute) unless the user opted in.
		const showRemote = experimentalStore.isEnabled("remoteTranscription");
		models = showRemote ? list : list.filter((m) => m.source !== "remote");
		if (!selectedModelId || !models.some((m) => m.id === selectedModelId)) {
			selectedModelId = pickDefaultModelId(models);
		}
	} catch (e) {
		toast.error(`Could not load caption models: ${e}`);
	}
}

onMount(() => {
	if (!asr) return;
	void refresh();
	asr.capabilities()
		.then((c) => (caps = c))
		.catch(() => {});
});

function pick(id: string) {
	selectedModelId = id;
	pickerOpen = false;
}

async function handleDownload(id: string) {
	if (!asr) return;
	downloadingId = id;
	downloadPct = 0;
	try {
		// Progress is scoped to this download's channel, so no model-id filtering.
		await asr.downloadModel(id, (p) => {
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
	if (!asr) return;
	try {
		await asr.deleteModel(id);
		await refresh();
	} catch (e) {
		toast.error(`Could not delete model: ${e}`);
	}
}

async function generate() {
	if (
		!asr ||
		!selected ||
		!selected.installed ||
		!selected.runnable ||
		!selected.runtimeAvailable ||
		!hasAudio
	)
		return;
	transcribing = true;
	cancelling = false;
	phase = "extracting";
	error = null;
	startedAt = Date.now();
	elapsedMs = 0;
	try {
		store.transcript = await asr.transcribe({
			audioPath: store.audioPath,
			microphonePath: store.microphonePath,
			modelId: selected.id,
			onPhase: (p) => {
				phase = p.phase;
			},
		});
	} catch (e) {
		// Keep whatever transcript is already loaded: this runs as "Regenerate"
		// with real data on screen, and `transcript` is not in the undo snapshot,
		// so clearing it here is unrecoverable.
		if (!`${e}`.includes(TRANSCRIBE_CANCELLED)) error = `${e}`;
	} finally {
		transcribing = false;
		cancelling = false;
		phase = "";
	}
}

async function cancelGenerate() {
	if (!asr?.cancel || cancelling) return;
	cancelling = true;
	try {
		await asr.cancel();
	} catch (e) {
		cancelling = false;
		error = `Couldn't stop transcription: ${e}`;
	}
}

async function exportSubs(format: "srt" | "vtt") {
	const t = store.transcript;
	if (!t || !captionFiles) return;
	try {
		// Map onto the output timeline (trim + cuts + speed) so cues line up with
		// the exported video, not the raw recording, using the same warp the export
		// dialog and Cloud track apply.
		await captionFiles.exportSidecar(toOutputTimeTranscript(store.timeMap, t), format);
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
const highlightOptions = [
	{ value: "none", label: "Off" },
	{ value: "active", label: "Active" },
	{ value: "progressive", label: "Progressive" },
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
// The theme picker preloads its own fonts; this only covers the applied style.
function applyPreset(style: Partial<CaptionStyle>) {
	store.updateCaptionStyle(style);
}
// The preset matching the current style exactly (so the picker shows the
// active theme), or null once the user has tweaked away from any preset.
const activeTheme = $derived.by(() => {
	const cs = store.captionStyle;
	return captionPresets.find((p) => captionStyleMatchesPreset(cs, p.value)) ?? null;
});

// The transcript line under the playhead, highlighted so you can follow along
// as it plays. Deliberately no auto-scroll: yanking the scroll position while
// someone is reading is worse than a still list.
const activeSegmentId = $derived.by(() => {
	const t = store.currentTime;
	const segs = store.transcript?.segments ?? [];
	let id: string | null = null;
	for (const s of segs) {
		if (s.start <= t) id = s.id;
		else break;
	}
	return id;
});

const visibleSegments = $derived(
	filterSegments(store.transcript?.segments ?? [], transcriptQuery),
);

// Transcription that SUCCEEDED but returned nothing. Distinct from an error
// (which sets `error`) and from a recording with no audio track at all
// (`hasAudio`): here the model ran and genuinely heard no speech: silence, or
// music/room tone only. Without this the panel just re-rendered its empty
// pre-transcribe self, which reads as "the button did nothing".
const noSpeechFound = $derived(
	!!store.transcript && store.transcript.segments.length === 0 && !error,
);
</script>

<div class="flex flex-col gap-4 animate-in fade-in duration-200">
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
          class="inline-flex items-center gap-1 rounded-full border border-border/50 bg-card/60 px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground"
          title={caps.gpu.name ?? gpuLabel}
        >
          {#if caps.gpu.available}
            <Zap size={9} class="text-muted-foreground" />
          {:else}
            <Cpu size={9} />
          {/if}
          {gpuLabel}
        </span>
      {/if}
    {/snippet}

    {#if caps && !caps.captionsAvailable && !hasRemoteModels}
      <!-- On-device captions aren't compiled into this build. Remote endpoints
           still work, so point the user at them rather than dead-ending. -->
      <div
        class="flex flex-col items-center gap-1.5 rounded-lg border border-dashed border-border/60 bg-card/40 px-4 py-6 text-center"
      >
        <Cpu size={20} class="text-muted-foreground" />
        <p class="text-[12px] font-medium text-foreground">
          On-device captions aren't available in this build
        </p>
        <p class="max-w-64 text-[11px] leading-relaxed text-muted-foreground">
          You can still caption through a remote endpoint: enable
          <span class="font-medium text-foreground">Remote transcription</span> in
          Settings &rsaquo; Advanced and add an OpenAI-compatible endpoint.
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
        <p class="max-w-60 text-[11px] leading-relaxed text-muted-foreground">
          This recording has no audio to transcribe. Record with your microphone or
          system audio on to generate captions.
        </p>
      </div>
    {:else}
    {#if caps && !caps.captionsAvailable}
      <!-- On-device engine absent from this build, but remote endpoints work.
           Explain why only remote models are selectable. -->
      <div
        class="mb-2 flex items-start gap-1.5 rounded-lg border border-warning/30 bg-warning/10 px-2.5 py-2 text-[11px] leading-tight text-warning"
      >
        <Cpu size={12} class="mt-px shrink-0" />
        <span>On-device models aren't available in this build. Only remote
          endpoints can transcribe here.</span>
      </div>
    {/if}
    <!-- Combobox selector: only the chosen model shows here; the full list
         lives in the popover so the tab stays compact. -->
    <Popover.Root open={pickerOpen} onOpenChange={(v) => (pickerOpen = v)}>
      <Popover.Trigger>
        {#snippet child({ props })}
          <button
            {...props as Record<string, unknown>}
            class="flex w-full items-center gap-2 rounded-lg bg-muted/60 px-2.5 py-2 text-left ring-1 ring-inset ring-border/40 transition-colors hover:bg-muted"
          >
            <span
              class={cn(
                "grid size-7 shrink-0 place-items-center rounded-md",
                selected?.installed && selected?.runnable
                  ? "bg-ink/5 text-ink"
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
                      {#if m.id === selectedModelId}<Check size={13} class="text-foreground" />{/if}
                    </span>
                    <span class="min-w-0 flex-1 truncate text-[12px]">{m.displayName}</span>
                    {#if m.recommended}
                      <span
                        class="shrink-0 rounded bg-muted px-1 py-0.5 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground"
                      >
                        Rec
                      </span>
                    {/if}
                    {#if !m.runnable}
                      <Lock size={11} class="shrink-0 text-muted-foreground/70" />
                    {:else if m.installed}
                      <Check size={11} class="shrink-0 text-success" />
                    {/if}
                    {#if m.approxSizeBytes}
                      <span class="shrink-0 text-[11px] tabular-nums text-muted-foreground">
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
      <div class="mt-2.5 border-t border-border/40 pt-2.5">
        <!-- Nine badges at 9px read as texture, not information. Only what
             decides the pick stays inline: language, size, and whether this
             machine can run it. The rest moved to one plain capability line. -->
        <div class="flex flex-wrap items-center gap-1.5 text-[11px]">
          <span class="font-medium text-foreground">{selected.family}</span>
          <span class="text-muted-foreground">·</span>
          <span class="text-muted-foreground">{langLabel(selected)}</span>
          {#if selected.approxSizeBytes}
            <span class="text-muted-foreground">·</span>
            <span class="tabular-nums text-muted-foreground">
              {formatSize(selected.approxSizeBytes)}
            </span>
          {/if}
          {#if selected.source === "extension"}
            <span
              class="rounded bg-muted px-1.5 py-0.5 text-[11px] font-medium text-muted-foreground"
            >
              Extension
            </span>
          {:else if selected.source === "remote"}
            <span class="rounded bg-warning/12 px-1.5 py-0.5 text-[11px] font-medium text-warning">
              Experimental
            </span>
          {/if}
          {#if selected.requiresGpu}
            <span
              class="rounded bg-destructive/10 px-1.5 py-0.5 text-[11px] font-medium text-destructive"
            >
              Needs GPU
            </span>
          {:else if selected.prefersGpu}
            <span class="rounded bg-warning/10 px-1.5 py-0.5 text-[11px] font-medium text-warning">
              Faster with GPU
            </span>
          {/if}
        </div>

        {#if capabilityLine}
          <p class="mt-1 text-[11px] text-muted-foreground">{capabilityLine}</p>
        {/if}

        <!-- Relative comparison bars. Editorial scores for ranking models
             against each other, so they're labelled as such rather than shown
             as a percentage or a benchmark figure. -->
        {#if selected.accuracyScore !== null || selected.speedScore !== null}
          <div class="mt-2 flex flex-col gap-1.5">
            <p class="text-[10px] uppercase tracking-wide text-muted-foreground/80">
              Compared with other models
            </p>
            {#each [{ label: "Accuracy", score: selected.accuracyScore }, { label: "Speed", score: selected.speedScore }] as bar (bar.label)}
              {#if bar.score !== null}
                <div class="flex items-center gap-2">
                  <span class="w-14 shrink-0 text-[11px] text-muted-foreground">{bar.label}</span>
                  <div
                    class="h-2 min-w-0 flex-1 overflow-hidden rounded-full bg-muted"
                    role="meter"
                    aria-label="{bar.label}, relative to other models"
                    aria-valuenow={bar.score}
                    aria-valuemin={0}
                    aria-valuemax={100}
                  >
                    <div class="h-full rounded-full bg-foreground" style="width: {bar.score}%"></div>
                  </div>
                  <span class="w-7 shrink-0 text-right text-[10px] tabular-nums text-muted-foreground">
                    {bar.score}
                  </span>
                </div>
              {/if}
            {/each}
          </div>
        {/if}

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
          {:else if selected.source === "remote"}
            <!-- Remote endpoints are managed in Settings; the panel only reflects
                 their readiness (key present) here. -->
            {#if selected.runtimeAvailable}
              <p class="flex items-center gap-1.5 text-[11px] font-medium text-success">
                <Check size={13} /> Endpoint ready
              </p>
            {:else}
              <p class="flex items-center gap-1.5 text-[11px] font-medium text-muted-foreground">
                <Lock size={12} /> Add an API key in Settings
              </p>
            {/if}
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
      <div class="flex gap-1.5">
        <Button
          variant="dark"
          size="sm"
          class="flex-1 gap-1.5"
          disabled={!selected?.installed ||
            !selected?.runnable ||
            !selected?.runtimeAvailable ||
            transcribing}
          onclick={generate}
        >
          {#if transcribing}
            <LoaderCircle size={14} class="animate-spin" />
            {phase === "extracting" ? "Reading audio" : "Transcribing"}
            <span class="tabular-nums opacity-70">{elapsedLabel(elapsedMs)}</span>
          {:else}
            <AiWand size={14} />
            {store.transcript ? "Regenerate captions" : "Generate captions"}
          {/if}
        </Button>
        {#if transcribing && asr?.cancel}
          <Button variant="outline" size="sm" disabled={cancelling} onclick={cancelGenerate}>
            {cancelling ? "Stopping…" : "Cancel"}
          </Button>
        {/if}
      </div>

      {#if usable.length === 0}
        <p class="mt-2 text-[11px] text-muted-foreground">
          Download a model your device can run to enable captioning.
        </p>
      {/if}

      {#if error}
        <div
          class="mt-2 flex items-start gap-1.5 rounded-md border border-warning/40 bg-warning/10 px-2 py-1.5 text-[11px] text-warning"
          role="alert"
        >
          <AlertTriangle size={12} class="mt-px shrink-0" />
          <span class="min-w-0">{error}</span>
        </div>
      {:else if noSpeechFound}
        <div
          class="mt-2 flex items-start gap-1.5 rounded-md border border-border/60 bg-muted/40 px-2 py-1.5 text-[11px] text-muted-foreground"
          role="status"
        >
          <Info size={12} class="mt-px shrink-0" />
          <span class="min-w-0">
            No speech found in this recording. If it's music or background noise only, there's
            nothing to caption. Otherwise, check that the right audio track was recorded, or try a
            larger model.
          </span>
        </div>
      {/if}
    </div>
    {/if}
  </PanelSection>

  {#if store.transcript && store.transcript.segments.length > 0}
    {@const cs = store.captionStyle}
    <!-- One master switch above the tabs, because it governs all three views:
         style and motion only matter if captions render at all. -->
    <SettingRow label="Show captions">
      {#snippet children(props)}
        <Switch
          checked={cs.enabled}
          {...props}
          onCheckedChange={(next: boolean) => store.updateCaptionStyle({ enabled: next })}
        />
      {/snippet}
    </SettingRow>

    <Tabs.Root value={view} onValueChange={(v: string) => setView(v as StyleView)}>
      <Tabs.List variant="soft" class="h-7 w-full p-0.5">
        {#each [{ value: "style", label: "Style" }, { value: "motion", label: "Motion" }, { value: "text", label: "Text" }] as t (t.value)}
          <Tabs.Trigger value={t.value} class="h-6 flex-1 text-[11px]">{t.label}</Tabs.Trigger>
        {/each}
      </Tabs.List>

      <Tabs.Content value="style" class="mt-3 flex flex-col gap-4 focus-visible:outline-none">
        <fieldset
          class="flex flex-col gap-3 disabled:opacity-50"
          disabled={!cs.enabled}
        >
          {#if captionPresets.length > 0}
            <div class="flex flex-col gap-1.5">
              <div class="flex items-baseline justify-between gap-2">
                <span class="text-[11px] text-foreground">Theme</span>
                {#if !activeTheme}
                  <span class="text-[11px] text-muted-foreground">Custom</span>
                {/if}
              </div>
              <CaptionThemePicker
                themes={captionPresets.map((p) => ({
                  id: p.id,
                  label: p.label,
                  value: p.value,
                }))}
                activeId={activeTheme?.id ?? null}
                onSelect={applyPreset}
              />
            </div>
          {/if}

          <PropRow label="Font">
            <FontPicker
              value={cs.fontFamily}
              weight={cs.fontWeight}
              onChange={(v) => store.updateCaptionStyle({ fontFamily: v })}
            />
          </PropRow>

          <PropRow label="Weight">
            <PropSelect
              class="flex-1"
              label="Font weight"
              value={String(cs.fontWeight)}
              options={FONT_WEIGHTS.map((w) => ({
                value: String(w.value),
                label: w.label,
              }))}
              onChange={(v) => store.updateCaptionStyle({ fontWeight: Number(v) })}
            />
          </PropRow>

          <SliderRow
            label="Font size"
            value={cs.fontSizePct}
            min={2}
            max={10}
            step={0.5}
            unit="%"
            onchange={(next) => store.updateCaptionStyle({ fontSizePct: next })}
            formatValue={(v) => `${v}%`}
          />

          <SettingRow label="Uppercase">
            {#snippet children(props)}
            <SegmentedToggle
              checked={cs.uppercase}
              offLabel="Off"
              onLabel="On"
              size="xs"
              {...props}
              onCheckedChange={(next) => store.updateCaptionStyle({ uppercase: next })}
            />
            {/snippet}
          </SettingRow>

          <PropRow label="Color">
            <ColorField
              dense
              hideLabel
              class="flex-1"
              label="Color"
              value={cs.color}
              swatches={CAPTION_SWATCHES}
              {recents}
              oncommit={(c) => {
                store.updateCaptionStyle({ color: c });
                rememberColor(c);
              }}
            />
          </PropRow>

          <PropRow label="Position">
            <PropSelect
              class="flex-1"
              label="Position"
              value={cs.position}
              options={positionOptions}
              onChange={(v) =>
                store.updateCaptionStyle({ position: v as "top" | "center" | "bottom" })}
            />
          </PropRow>

          <PropRow label="Align">
            <div class="contents">
              {#snippet alignLeftIcon()}<AlignLeft size={12} />{/snippet}
              {#snippet alignCenterIcon()}<AlignCenter size={12} />{/snippet}
              {#snippet alignRightIcon()}<AlignRight size={12} />{/snippet}
              <Segmented
                size="xs"
                fill={false}
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
          </PropRow>

          {#if cs.position !== "center"}
            <SliderRow
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
        </fieldset>

        <PanelSection
          title="Background & outline"
          hint="Keep captions legible over any footage: a backing box behind the text, or a stroke around it."
          flush
          collapsible
          defaultOpen={false}
        >
          <fieldset class="flex flex-col gap-3 disabled:opacity-50" disabled={!cs.enabled}>
            <PropRow label="Background">
              <PropSelect
                class="flex-1"
                label="Background"
                value={cs.background}
                options={backgroundOptions}
                onChange={(v) =>
                  store.updateCaptionStyle({ background: v as "none" | "soft" | "box" })}
              />
            </PropRow>

            {#if cs.background === "box"}
              <PropRow label="Box">
                <ColorField
                  dense
                  hideLabel
                  class="flex-1"
                  label="Box color"
                  value={cs.backgroundColor}
                  swatches={CAPTION_SWATCHES}
                  {recents}
                  oncommit={(c) => {
                    store.updateCaptionStyle({ backgroundColor: c });
                    rememberColor(c);
                  }}
                />
              </PropRow>

              <SliderRow
                label="Box opacity"
                value={cs.backgroundOpacity}
                min={0}
                max={100}
                step={5}
                unit="%"
                onchange={(next) => store.updateCaptionStyle({ backgroundOpacity: next })}
                formatValue={(v) => `${v}%`}
              />

              <SliderRow
                label="Corner radius"
                value={cs.boxRadiusEm}
                min={0}
                max={2}
                step={0.05}
                unit="em"
                onchange={(next) => store.updateCaptionStyle({ boxRadiusEm: next })}
                formatValue={(v) => (v >= 1.2 ? "Pill" : v === 0 ? "Square" : v.toFixed(2))}
              />

              <SliderRow
                label="Padding"
                value={cs.boxPaddingXEm}
                min={0}
                max={2}
                step={0.05}
                unit="em"
                onchange={(next) => store.updateCaptionStyle({ boxPaddingXEm: next })}
                formatValue={(v) => `${v.toFixed(2)}em`}
              />
            {/if}

            <SliderRow
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
              <PropRow label="Outline">
                <ColorField
                  dense
                  hideLabel
                  class="flex-1"
                  label="Outline color"
                  value={cs.outlineColor}
                  swatches={CAPTION_SWATCHES}
                  {recents}
                  oncommit={(c) => {
                    store.updateCaptionStyle({ outlineColor: c });
                    rememberColor(c);
                  }}
                />
              </PropRow>
            {/if}
          </fieldset>
        </PanelSection>

        <PanelSection
          title="Spacing & wrapping"
          hint="How the text breaks across lines and how tightly it sits."
          flush
          collapsible
          defaultOpen={false}
        >
          <fieldset class="flex flex-col gap-3 disabled:opacity-50" disabled={!cs.enabled}>
            <SliderRow
              label="Max lines"
              value={cs.maxLines}
              min={1}
              max={4}
              step={1}
              unit=""
              onchange={(next) => store.updateCaptionStyle({ maxLines: next })}
              formatValue={(v) => `${v}`}
            />

            <SliderRow
              label="Wrap width"
              value={cs.maxCharsPerLine}
              min={16}
              max={80}
              step={1}
              unit=""
              onchange={(next) => store.updateCaptionStyle({ maxCharsPerLine: next })}
              formatValue={(v) => `${v} chars`}
            />

            <SliderRow
              label="Line height"
              value={cs.lineHeight}
              min={1}
              max={2}
              step={0.05}
              unit=""
              onchange={(next) => store.updateCaptionStyle({ lineHeight: next })}
              formatValue={(v) => v.toFixed(2)}
            />

            <SliderRow
              label="Letter spacing"
              value={cs.letterSpacing}
              min={-0.05}
              max={0.3}
              step={0.01}
              unit="em"
              onchange={(next) => store.updateCaptionStyle({ letterSpacing: next })}
              formatValue={(v) => `${v.toFixed(2)}em`}
            />
          </fieldset>
        </PanelSection>
      </Tabs.Content>

      <Tabs.Content value="motion" class="mt-3 flex flex-col gap-3 focus-visible:outline-none">
        {@const ca = resolveCaptionAnimation(cs.animation)}
        <p class="text-[11px] leading-snug text-muted-foreground">
          Reveal and highlight, synced to speech. A word-timestamped model gives the tightest sync.
        </p>
        <fieldset
          class="flex flex-col gap-3 disabled:opacity-50"
          disabled={!cs.enabled}
        >
          <PropRow label="Show">
            <PropSelect
              class="flex-1"
              label="Show"
              value={ca.chunk}
              options={chunkOptions}
              onChange={(v) => updateAnimation({ chunk: v as CaptionAnimation["chunk"] })}
            />
          </PropRow>

          {#if ca.chunk === "phrase"}
            <SliderRow
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

          <PropRow label="Highlight">
            <PropSelect
              class="flex-1"
              label="Highlight"
              value={ca.highlight ?? "none"}
              options={highlightOptions}
              onChange={(v) =>
                updateAnimation({ highlight: v as CaptionAnimation["highlight"] })}
            />
          </PropRow>

          {#if (ca.highlight ?? "none") === "progressive"}
            <PropRow label="Unspoken">
              <ColorField
                dense
                hideLabel
                class="flex-1"
                label="Unspoken color"
                value={cs.mutedColor}
                swatches={CAPTION_SWATCHES}
                {recents}
                oncommit={(c) => {
                  store.updateCaptionStyle({ mutedColor: c });
                  rememberColor(c);
                }}
              />
            </PropRow>
          {/if}

          <PropRow label="Active word">
            <PropSelect
              class="flex-1"
              label="Active word"
              value={ca.emphasis}
              options={emphasisOptions}
              onChange={(v) => updateAnimation({ emphasis: v as CaptionAnimation["emphasis"] })}
            />
          </PropRow>

          {#if ca.emphasis === "color"}
            <PropRow label="Highlight">
              <ColorField
                dense
                hideLabel
                class="flex-1"
                label="Highlight color"
                value={ca.emphasisColor}
                swatches={CAPTION_SWATCHES}
                {recents}
                oncommit={(c) => {
                  updateAnimation({ emphasisColor: c });
                  rememberColor(c);
                }}
              />
            </PropRow>
          {/if}

          <PropRow label="Entrance">
            <PropSelect
              class="flex-1"
              label="Entrance"
              value={ca.entrance}
              options={entranceOptions}
              onChange={(v) => updateAnimation({ entrance: v as CaptionAnimation["entrance"] })}
            />
          </PropRow>

          {#if ca.entrance !== "none"}
            <SliderRow
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
            <PropRow label="In pauses">
              <PropSelect
                class="flex-1"
                label="In pauses"
                value={ca.holdGaps ? "hold" : "clear"}
                options={holdOptions}
                onChange={(v) => updateAnimation({ holdGaps: v === "hold" })}
              />
            </PropRow>
          {/if}
        </fieldset>
      </Tabs.Content>

      <Tabs.Content value="text" class="mt-3 flex flex-col gap-2 focus-visible:outline-none">
        <div class="flex items-center gap-1.5 rounded-lg border border-border/60 bg-card/40 px-2">
          <Search size={13} class="shrink-0 text-muted-foreground" />
          <input
            bind:value={transcriptQuery}
            placeholder="Search transcript…"
            aria-label="Search transcript"
            onkeydown={(e) => {
              if (e.key === "Escape" && transcriptQuery) {
                e.stopPropagation();
                transcriptQuery = "";
              }
            }}
            class="h-7 w-full bg-transparent text-[11px] outline-none placeholder:text-muted-foreground"
          />
          {#if transcriptQuery}
            <button
              type="button"
              aria-label="Clear search"
              class="shrink-0 rounded p-0.5 text-muted-foreground hover:text-foreground"
              onclick={() => (transcriptQuery = "")}
            >
              <X size={12} />
            </button>
          {/if}
        </div>

        <div class="flex items-center justify-between gap-2">
          <p class="min-w-0 truncate text-[11px] text-muted-foreground" aria-live="polite">
            {#if transcriptQuery.trim()}
              {visibleSegments.length} of {store.transcript.segments.length} lines
            {:else}
              Click a line to jump the playhead.
            {/if}
          </p>
          <div class="flex shrink-0 items-center gap-1">
            <Button variant="ghost" size="xs" class="h-6 gap-1 text-[10px]" onclick={() => exportSubs("srt")}>
              <FileDown size={11} /> SRT
            </Button>
            <Button variant="ghost" size="xs" class="h-6 gap-1 text-[10px]" onclick={() => exportSubs("vtt")}>
              <FileDown size={11} /> VTT
            </Button>
          </div>
        </div>

        <div class="flex flex-col gap-0.5">
          {#each visibleSegments as seg (seg.id)}
            {@const isActive = seg.id === activeSegmentId}
            <button
              type="button"
              aria-current={isActive ? "true" : undefined}
              class={cn(
                "group flex items-start gap-2 rounded-md px-1.5 py-1 text-left transition-colors",
                isActive ? "bg-foreground/10" : "hover:bg-muted/60",
              )}
              onclick={() => store.seek(seg.start)}
            >
              <span
                class={cn(
                  "shrink-0 pt-px font-mono text-[11px] tabular-nums",
                  isActive
                    ? "text-foreground"
                    : "text-muted-foreground/70 group-hover:text-foreground",
                )}
              >
                {clock(seg.start)}
              </span>
              <span
                class={cn(
                  "min-w-0 text-[12px] leading-snug text-foreground",
                  isActive && "font-medium",
                )}>{seg.text}</span>
            </button>
          {/each}

          {#if visibleSegments.length === 0}
            <p class="px-1.5 py-4 text-center text-[11px] text-muted-foreground">
              No lines match "{transcriptQuery.trim()}".
            </p>
          {/if}
        </div>
      </Tabs.Content>
    </Tabs.Root>
  {/if}
</div>
