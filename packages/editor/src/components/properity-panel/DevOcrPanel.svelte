<script lang="ts">
// Dev-only review surface for the on-device OCR pass: the run's progress, plus every frame it kept and the text read off it.

import { Download, FlaskConical, ImageOff, RotateCw, ScanText } from "@recast/icons";
import { Badge } from "@recast/ui/badge";
import { Button } from "@recast/ui/button";
import { Progress } from "@recast/ui/progress";
import { toast } from "@recast/ui/sonner";
import {
	getEditorServices,
	type OcrProgress,
	type VideoTextTimeline,
} from "../../lib/editor/services";
import { clock } from "../../lib/format/time";
import type { ScreenStateSpan } from "../../lib/wire-types";
import type { EditorStore } from "../../stores/editor-store.svelte";
import {
	etaLabel,
	exportBodyFor,
	exportFilename,
	phaseDetail,
	phaseTitle,
	progressValue,
	type RunStatus,
	spanGist,
	summaryRows,
} from "./dev-ocr-panel.logic";
import OcrFrameDialog from "./OcrFrameDialog.svelte";
import PanelSection from "./PanelSection.svelte";

interface Props {
	store: EditorStore;
}
let { store }: Props = $props();

const ocr = getEditorServices().ocr;

let status = $state<RunStatus>("idle");
let progress = $state<OcrProgress | null>(null);
let error = $state<string | null>(null);
let timeline = $state<VideoTextTimeline | null>(null);
let elapsedMs = $state(0);
let inspecting = $state<ScreenStateSpan | null>(null);

// Elapsed for the CURRENT phase: an OCR frame costs orders of magnitude more than a decode frame.
let phaseStartedAt = 0;
let phaseElapsedMs = $state(0);
let ticker: ReturnType<typeof setInterval> | null = null;

// The raw .mp4, not the .recast container; camera, background and annotations composite at export, so OCR sees the screen only.
const mediaPath = $derived(store.recordingPath);

// The kept footage in original seconds, so trimmed and cut regions are never read or paid for.
const keptRanges = $derived(store.segments.map((s) => [s.start, s.end] as [number, number]));

const percent = $derived(progressValue(progress));
const eta = $derived(etaLabel(phaseElapsedMs, progress));
const rows = $derived(timeline ? summaryRows(timeline.stats, timeline.spans.length) : []);

function onProgress(p: OcrProgress) {
	if (p.phase !== progress?.phase) {
		phaseStartedAt = performance.now();
		phaseElapsedMs = 0;
	}
	progress = p;
}

async function run() {
	if (!mediaPath || !ocr) return;
	status = "running";
	error = null;
	timeline = null;
	progress = null;
	phaseStartedAt = performance.now();
	phaseElapsedMs = 0;
	// Drives the ETA between backend ticks, so the estimate counts down through a slow frame.
	ticker = setInterval(() => {
		phaseElapsedMs = performance.now() - phaseStartedAt;
	}, 250);

	const started = performance.now();
	try {
		timeline = await ocr.readVideoText({
			videoPath: mediaPath,
			previews: true,
			includeRanges: keptRanges,
			onPhase: onProgress,
		});
		elapsedMs = Math.round(performance.now() - started);
		status = "ready";
	} catch (e) {
		error = `${e}`;
		status = "error";
	} finally {
		if (ticker) clearInterval(ticker);
		ticker = null;
		progress = null;
	}
}

let exporting = $state(false);

// The save dialog's filter picks JSON or Markdown, and the chosen path's extension decides how it serializes.
async function exportRead() {
	if (!timeline || exporting || !ocr) return;
	exporting = true;
	try {
		const name = exportFilename("json");
		await ocr.exportScreenText(exportBodyFor(name, timeline, clock), name);
		toast.success("Exported screen text");
	} catch (e) {
		toast.error(`Export failed: ${e}`);
	} finally {
		exporting = false;
	}
}

$effect(() => () => {
	if (ticker) clearInterval(ticker);
});
</script>

<div class="flex flex-col gap-3">
  <PanelSection
    title="Screen text"
    hint="Samples the frames where the screen changed, reads the text on each with on-device OCR, then collapses neighbouring frames that read the same into one screen state."
  >
    {#if !mediaPath}
      <p class="text-muted-foreground text-xs">No source recording is loaded.</p>
    {:else}
      <Button
        class="w-full"
        variant="secondary"
        size="sm"
        disabled={status === "running"}
        onclick={run}
      >
        {#if status === "running"}
          <RotateCw class="size-4 animate-spin" />
          Reading…
        {:else if timeline}
          <RotateCw class="size-4" />
          Read again
        {:else}
          <ScanText class="size-4" />
          Read screen text
        {/if}
      </Button>
    {/if}

    <!-- The work, made visible: which stage, counting what, how far through, and
         how much longer. Never an unbounded spinner. -->
    {#if status === "running"}
      <div class="mt-3 flex flex-col gap-1.5" aria-live="polite" aria-atomic="true">
        <div class="flex items-baseline justify-between gap-2">
          <span class="text-xs font-medium">{phaseTitle(progress)}</span>
          <span class="text-muted-foreground text-[10px] tabular-nums">
            {percent === null ? "" : `${Math.round(percent)}%`}
          </span>
        </div>
        <Progress value={percent} />
        <div class="text-muted-foreground flex items-baseline justify-between gap-2 text-[10px]">
          <span class="tabular-nums">{phaseDetail(progress)}</span>
          <span class="tabular-nums">{eta}</span>
        </div>
      </div>
    {/if}

    {#if status === "error" && error}
      <p class="text-destructive mt-2 text-xs" role="alert">{error}</p>
    {/if}
  </PanelSection>

  {#if status === "ready" && timeline}
    <!-- The receipt for the run. Where the time went and how much of the video was
         actually looked at, so a slow or thin read can be attributed, not guessed at. -->
    <PanelSection title="This read" collapsible defaultOpen={false}>
      <dl class="grid grid-cols-2 gap-x-3 gap-y-2">
        {#each rows as row (row.label)}
          <div class="flex flex-col" title={row.hint}>
            <dt class="text-muted-foreground text-[10px]">{row.label}</dt>
            <dd class="text-xs font-medium tabular-nums">{row.value}</dd>
          </div>
        {/each}
      </dl>
      <p class="text-muted-foreground mt-2.5 text-[10px] leading-relaxed">
        {timeline.engine} · {(elapsedMs / 1000).toFixed(1)}s total
      </p>
    </PanelSection>

    <PanelSection
      title="Screen states"
      hint="Each row is a stretch of time where the screen text stayed the same. Open one to see exactly what was read and where."
      flush
    >
      {#snippet action()}
        <Button
          variant="ghost"
          size="xs"
          disabled={exporting}
          onclick={exportRead}
          title="Export the whole read as JSON or Markdown"
        >
          <Download class="size-3.5" />
          Export
        </Button>
      {/snippet}
      {#if timeline.spans.length === 0}
        <p class="text-muted-foreground text-xs">
          No text was found in the footage you kept.
        </p>
      {:else}
        <ul class="flex flex-col gap-2">
          {#each timeline.spans as span (span.start)}
            <li
              class="border-border hover:border-primary/60 overflow-hidden rounded-lg border transition-colors"
            >
              <!-- The frame opens the inspector; the caption seeks. Two separate
                   targets, because they are two separate intents. -->
              <button
                type="button"
                class="focus-visible:ring-ring group relative block w-full focus-visible:ring-2 focus-visible:outline-none"
                onclick={() => (inspecting = span)}
                aria-label="Inspect the {span.elements.length} elements read at {clock(span.start)}"
              >
                {#if span.preview}
                  <img
                    src={span.preview}
                    alt="Frame read at {clock(span.start)}"
                    class="block w-full"
                    loading="lazy"
                  />
                {:else}
                  <div
                    class="text-muted-foreground bg-muted flex aspect-video items-center justify-center gap-1.5 text-[10px]"
                  >
                    <ImageOff class="size-3.5" />
                    No preview
                  </div>
                {/if}
                <span
                  class="bg-background/85 absolute inset-x-0 bottom-0 hidden items-center justify-center py-1 text-[10px] font-medium backdrop-blur-sm group-hover:flex group-focus-visible:flex"
                >
                  Inspect frame
                </span>
              </button>

              <div class="flex flex-col gap-1 p-2">
                <div class="flex items-center justify-between gap-2">
                  <button
                    type="button"
                    class="hover:text-primary focus-visible:ring-ring rounded text-xs font-medium tabular-nums focus-visible:ring-2 focus-visible:outline-none"
                    onclick={() => store.seek(span.start)}
                    title="Jump to {clock(span.start)}"
                  >
                    {clock(span.start)} – {clock(span.end)}
                  </button>
                  <Badge variant="secondary" class="h-4 shrink-0 px-1 text-[9px] tabular-nums">
                    {span.elements.length}
                    {span.elements.length === 1 ? "element" : "elements"}
                  </Badge>
                </div>
                <p class="text-muted-foreground line-clamp-2 text-[11px] leading-snug">
                  {spanGist(span)}
                </p>
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    </PanelSection>
  {/if}

  {#if status === "idle"}
    <p
      class="text-muted-foreground flex items-center gap-1.5 px-1 text-[11px] leading-relaxed"
    >
      <FlaskConical class="size-3.5 shrink-0" />
      Experimental. Accuracy on dense UI text is not yet validated.
    </p>
  {/if}
</div>

<OcrFrameDialog
  span={inspecting}
  open={inspecting !== null}
  onOpenChange={(open) => {
    if (!open) inspecting = null;
  }}
  onSeek={(t) => store.seek(t)}
/>
