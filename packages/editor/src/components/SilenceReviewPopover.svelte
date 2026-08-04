<script lang="ts">
import { getEditorServices, type SilenceSegment } from "../lib/editor/services";
import type { EditorStore } from "../stores/editor-store.svelte";
import { overlapsAny } from "../lib/timeline/cuts";
import { clockDecis as formatTime, compactDuration as formatDuration } from "../lib/format/time";
import {
	BULK_MIN_CONFIDENCE,
	confidenceBarClass,
	confidenceLabel,
	confidenceTextClass,
	cutBounds,
	parseSensitivity,
	SENSITIVITY_OPTIONS,
	SENSITIVITY_PRESETS,
	type Sensitivity,
} from "./silence-review.logic";
import {
	AlertTriangle,
	Check,
	Eye,
	RotateCcw,
	Scissors,
	Sparkles,
	VolumeX,
	XCircle,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import * as Tooltip from "@recast/ui/tooltip";
import { cn } from "@recast/ui/utils";
import { safeStorage } from "@recast/ui/persisted-state";

interface Props {
	store: EditorStore;
	onclose: () => void;
}

let { store, onclose }: Props = $props();

// Persisted across sessions (presets/options live in silence-review.logic).
const SENSITIVITY_KEY = "recast-silence-sensitivity";

let sensitivity = $state<Sensitivity>(
	parseSensitivity(safeStorage.get<string>(SENSITIVITY_KEY, "")),
);

function setSensitivity(next: Sensitivity) {
	if (next === sensitivity) return;
	sensitivity = next;
	safeStorage.set(SENSITIVITY_KEY, next);
}

type Status = "idle" | "loading" | "ready" | "error" | "empty";
let status = $state<Status>("idle");
let errorMsg = $state<string | null>(null);
// Suggestions the user hasn't yet cut or dismissed.
let pending = $state<SilenceSegment[]>([]);

// Re-runs whenever `sensitivity` changes.
$effect(() => {
	void sensitivity;
	void loadSuggestions();
});

async function loadSuggestions() {
	if (!store.audioPath && !store.microphonePath) {
		status = "error";
		errorMsg = "This clip has no audio track to analyse.";
		return;
	}
	status = "loading";
	errorMsg = null;
	try {
		const analysis = getEditorServices().analysis;
		const result = analysis
			? await analysis.detectSilence({
					audioPath: store.audioPath,
					microphonePath: store.microphonePath,
					cursorPath: store.cursorPath,
					options: SENSITIVITY_PRESETS[sensitivity],
				})
			: [];
		// Drop anything already removed by a cut, or previously dismissed,
		// then surface the strongest candidates first.
		pending = result
			.filter(
				(s) =>
					!overlapsAny(store.cuts, s.start, s.end) &&
					!overlapsAny(store.dismissedSilences, s.start, s.end),
			)
			.sort((a, b) => b.confidence - a.confidence);
		status = pending.length === 0 ? "empty" : "ready";
	} catch (err) {
		console.error("Failed to detect silence", err);
		errorMsg = err instanceof Error ? err.message : String(err);
		status = "error";
	}
}

// Zoom regions and annotations a cut must not bisect: splitting one would
// need overlay-time surgery the MVP intentionally avoids.
const blockers = $derived.by(() => [
	...store.zoomRegions.map((z) => ({ start: z.start, end: z.end })),
	...store.annotations.map((a) => ({ start: a.start, end: a.end })),
	...store.cuts.map((c) => ({ start: c.start, end: c.end })),
]);

function isBlocked(seg: SilenceSegment): boolean {
	return overlapsAny(blockers, seg.start, seg.end);
}

function previewAt(seg: SilenceSegment) {
	store.seek(seg.start);
}

/** Apply the keep-margin and commit a cut. Returns true if a cut landed. */
function commitCut(seg: SilenceSegment): boolean {
	const b = cutBounds(seg.start, seg.end);
	if (!b) return false;
	return store.addCut(b.start, b.end, "silence") !== null;
}

function cut(seg: SilenceSegment) {
	if (isBlocked(seg)) return;
	commitCut(seg);
	pending = pending.filter((s) => s !== seg);
	if (pending.length === 0) status = "empty";
}

function dismiss(seg: SilenceSegment) {
	store.dismissSilence(seg.start, seg.end);
	pending = pending.filter((s) => s !== seg);
	if (pending.length === 0) status = "empty";
}

function cutAll() {
	const remaining: SilenceSegment[] = [];
	for (const seg of pending) {
		if (isBlocked(seg) || seg.confidence < BULK_MIN_CONFIDENCE) {
			remaining.push(seg);
			continue;
		}
		commitCut(seg);
	}
	pending = remaining;
	if (pending.length === 0) status = "empty";
}

function dismissAll() {
	for (const seg of pending) store.dismissSilence(seg.start, seg.end);
	pending = [];
	status = "empty";
}

// Clear dismissed decisions and re-scan so rejected suggestions re-surface.
function resetDismissedAndRescan() {
	store.clearDismissedSilences();
	void loadSuggestions();
}

const bulkCount = $derived(
	pending.filter((s) => !isBlocked(s) && s.confidence >= BULK_MIN_CONFIDENCE).length,
);
const totalRecoverable = $derived(
	pending.filter((s) => !isBlocked(s)).reduce((sum, s) => sum + (s.end - s.start), 0),
);
</script>

<div
  role="dialog"
  aria-label="Silence suggestions"
  class="flex max-h-[60vh] w-80 flex-col overflow-hidden rounded-md border border-border bg-popover text-popover-foreground shadow-xl ring-1 ring-border"
>
  <header class="flex items-center justify-between gap-2 border-b border-border px-3 py-2">
    <div class="flex items-center gap-1.5">
      <Scissors size={13} class="text-primary" />
      <h3 class="text-[11px] font-semibold tracking-tight">Remove silence</h3>
    </div>
    <Button variant="ghost" size="xs" onclick={onclose} class="gap-1.5">
      <XCircle size={11} />
      Close
    </Button>
  </header>

  <div class="flex items-center gap-1.5 border-b border-border px-3 py-1.5">
    <span class="text-[10px] font-medium text-muted-foreground">Sensitivity</span>
    <div
      class="ml-auto flex items-center gap-0.5 rounded-md bg-muted/60 p-0.5 ring-1 ring-inset ring-border/40"
    >
      {#each SENSITIVITY_OPTIONS as opt (opt.id)}
        <button
          type="button"
          onclick={() => setSensitivity(opt.id)}
          class={cn(
            "rounded px-1.5 py-0.5 text-[10px] font-semibold transition-colors",
            sensitivity === opt.id
              ? "bg-card text-foreground shadow-(--shadow-craft-inset)"
              : "text-muted-foreground hover:text-foreground",
          )}
        >
          {opt.label}
        </button>
      {/each}
    </div>
  </div>

  {#if status === "loading"}
    <div class="flex items-center justify-center gap-2 px-3 py-6 text-[11px] text-muted-foreground">
      <div class="size-3 animate-spin rounded-full border border-muted-foreground/40 border-t-foreground"></div>
      Analysing audio &amp; motion…
    </div>
  {:else if status === "error"}
    <div class="flex flex-col gap-2 px-3 py-3 text-[11px] text-muted-foreground">
      <p>{errorMsg ?? "Could not analyse this clip."}</p>
      <Button variant="secondary" size="xs" onclick={loadSuggestions}>Retry</Button>
    </div>
  {:else if status === "empty"}
    <div class="flex flex-col items-center gap-1 px-3 py-6 text-center text-[11px] text-muted-foreground">
      <Sparkles size={14} class="text-muted-foreground/70" />
      <p class="font-medium text-foreground">No silence to remove</p>
      <p>Nothing left to review on this clip.</p>
      <div class="mt-1 flex items-center gap-1">
        <Button variant="ghost" size="xs" onclick={loadSuggestions}>Re-scan</Button>
        {#if store.dismissedSilences.length > 0}
          <Button
            variant="ghost"
            size="xs"
            class="gap-1.5"
            onclick={resetDismissedAndRescan}
            title="Bring back {store.dismissedSilences.length} previously dismissed suggestion{store.dismissedSilences.length === 1 ? '' : 's'}"
          >
            <RotateCcw size={11} />
            Reset dismissed ({store.dismissedSilences.length})
          </Button>
        {/if}
      </div>
    </div>
  {:else if status === "ready"}
    <div class="flex items-center justify-between gap-2 border-b border-border px-3 py-1.5 text-[10px] text-muted-foreground">
      <span>
        {pending.length} found · {formatDuration(totalRecoverable)} recoverable
      </span>
      <div class="flex items-center gap-1">
        {#if store.dismissedSilences.length > 0}
          <Button
            variant="ghost"
            size="xs"
            class="gap-1"
            onclick={resetDismissedAndRescan}
            title="Bring back {store.dismissedSilences.length} previously dismissed suggestion{store.dismissedSilences.length === 1 ? '' : 's'}"
          >
            <RotateCcw size={10} />
            Reset ({store.dismissedSilences.length})
          </Button>
        {/if}
        <Button variant="ghost" size="xs" onclick={dismissAll}>Dismiss all</Button>
        <Button
          variant="default"
          size="xs"
          class="gap-1.5"
          onclick={cutAll}
          disabled={bulkCount === 0}
        >
          <Scissors size={11} />
          Cut all ({bulkCount})
        </Button>
      </div>
    </div>
    <ul class="flex-1 overflow-y-auto">
      {#each pending as seg (seg.start + "-" + seg.end)}
        {@const blocked = isBlocked(seg)}
        <li>
          <div
            class={cn(
              "group flex w-full items-center gap-2 border-b border-border px-3 py-2 text-left transition-colors",
              "hover:bg-muted/50",
              blocked && "opacity-60",
            )}
          >
            <span
              class={cn(
                "flex size-7 shrink-0 items-center justify-center rounded-md border bg-card",
                blocked ? "border-warning/40" : "border-border",
              )}
            >
              <VolumeX size={12} class={blocked ? "text-warning" : "text-primary"} />
            </span>
            <div class="min-w-0 flex-1">
              <div class="flex items-baseline justify-between gap-2">
                <span class="truncate text-[11px] font-medium text-foreground">
                  {formatDuration(seg.end - seg.start)} silent
                </span>
                <span class="font-mono text-[10px] tabular-nums text-muted-foreground">
                  {formatTime(seg.start)}
                </span>
              </div>
              <div class="truncate text-[10px] text-muted-foreground">
                {#if blocked}
                  <span class="inline-flex items-center gap-1 text-warning">
                    <AlertTriangle size={9} />
                    Overlaps a focus or annotation
                  </span>
                {:else}
                  <span class="inline-flex items-center gap-1 align-middle">
                    <span class="inline-block h-1 w-6 overflow-hidden rounded-full bg-muted">
                      <span
                        class={cn("block h-full rounded-full", confidenceBarClass(seg.confidence))}
                        style="width: {Math.round(seg.confidence * 100)}%"
                      ></span>
                    </span>
                    <span class={cn("font-medium", confidenceTextClass(seg.confidence))}>
                      {confidenceLabel(seg.confidence)}
                    </span>
                  </span>
                  · {[
                    seg.micSilent && "mic",
                    seg.systemSilent && "audio",
                    seg.cursorIdle && "cursor",
                  ]
                    .filter(Boolean)
                    .join(" + ")}
                {/if}
              </div>
            </div>
            <div class="flex shrink-0 items-center gap-1">
              <Button
                variant="ghost"
                size="xs"
                aria-label="Preview"
                onclick={() => previewAt(seg)}
              >
                <Eye size={11} />
              </Button>
              <Button
                variant="ghost"
                size="xs"
                aria-label="Dismiss"
                onclick={() => dismiss(seg)}
              >
                <XCircle size={11} />
              </Button>
              {#if blocked}
                <Tooltip.Root>
                  <Tooltip.Trigger>
                    <Button variant="default" size="xs" class="gap-1" disabled aria-label="Cut (blocked)">
                      <Check size={11} />
                    </Button>
                  </Tooltip.Trigger>
                  <Tooltip.Content>Remove the overlapping focus or annotation first</Tooltip.Content>
                </Tooltip.Root>
              {:else}
                <Button
                  variant="default"
                  size="xs"
                  class="gap-1"
                  aria-label="Cut this silence"
                  onclick={() => cut(seg)}
                >
                  <Check size={11} />
                </Button>
              {/if}
            </div>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</div>
