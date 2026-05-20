<script lang="ts">
  import {
    detectSilence,
    type SilenceDetectOptions,
    type SilenceSegment,
  } from "$lib/ipc";
  import type { EditorStore } from "$lib/stores/editor-store.svelte";
  import { overlapsAny } from "$lib/timeline/cuts";
  import {
    AlertTriangle,
    Check,
    Eye,
    RotateCcw,
    Scissors,
    Sparkles,
    VolumeX,
    XCircle,
  } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import * as Tooltip from "@recast/ui/tooltip";
  import { cn } from "@recast/ui/utils";

  interface Props {
    store: EditorStore;
    onclose: () => void;
  }

  let { store, onclose }: Props = $props();

  // A small margin kept at each end of a cut so speech onsets/tails right
  // beside the silence are never clipped.
  const CUT_PADDING = 0.12;
  // Bulk "Cut all" only takes confident suggestions — uncertain ones are
  // left for the user to judge individually.
  const BULK_MIN_CONFIDENCE = 0.5;

  // Detection sensitivity. "Balanced" uses the Rust-side defaults; the other
  // two trade recall against false positives. Persisted across sessions.
  type Sensitivity = "relaxed" | "balanced" | "aggressive";
  const SENSITIVITY_KEY = "recast-silence-sensitivity";
  const PRESETS: Record<Sensitivity, SilenceDetectOptions> = {
    relaxed: {
      flatnessDb: 3,
      minAudioSilence: 1,
      minSegment: 1.5,
    },
    balanced: {},
    aggressive: {
      flatnessDb: 8,
      minAudioSilence: 0.4,
      minSegment: 0.6,
    },
  };
  const SENSITIVITY_OPTIONS: Array<{ id: Sensitivity; label: string }> = [
    { id: "relaxed", label: "Relaxed" },
    { id: "balanced", label: "Balanced" },
    { id: "aggressive", label: "Aggressive" },
  ];

  function loadSensitivity(): Sensitivity {
    const v =
      typeof localStorage !== "undefined"
        ? localStorage.getItem(SENSITIVITY_KEY)
        : null;
    return v === "relaxed" || v === "aggressive" ? v : "balanced";
  }

  let sensitivity = $state<Sensitivity>(loadSensitivity());

  function setSensitivity(next: Sensitivity) {
    if (next === sensitivity) return;
    sensitivity = next;
    try {
      localStorage.setItem(SENSITIVITY_KEY, next);
    } catch {
      // localStorage unavailable — sensitivity just won't persist.
    }
    // The load effect re-runs because it reads `sensitivity`.
  }

  type Status = "idle" | "loading" | "ready" | "error" | "empty";
  let status = $state<Status>("idle");
  let errorMsg = $state<string | null>(null);
  // Suggestions the user hasn't yet cut or dismissed.
  let pending = $state<SilenceSegment[]>([]);

  // Re-runs whenever `sensitivity` changes (it is read below).
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
      const result = await detectSilence(
        store.audioPath,
        store.microphonePath,
        store.cursorPath,
        PRESETS[sensitivity],
      );
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

  function formatTime(s: number): string {
    const m = Math.floor(s / 60);
    const rem = s - m * 60;
    return `${m}:${rem.toFixed(1).padStart(4, "0")}`;
  }

  function formatDuration(s: number): string {
    return s >= 1 ? `${s.toFixed(1)}s` : `${Math.round(s * 1000)}ms`;
  }

  // Zoom regions and annotations a cut must not bisect — splitting one would
  // need overlay-time surgery the MVP intentionally avoids.
  const blockers = $derived.by(() => [
    ...store.zoomRegions.map((z) => ({ start: z.start, end: z.end })),
    ...store.annotations.map((a) => ({ start: a.start, end: a.end })),
    ...store.cuts.map((c) => ({ start: c.start, end: c.end })),
  ]);

  function isBlocked(seg: SilenceSegment): boolean {
    return overlapsAny(blockers, seg.start, seg.end);
  }

  function confidenceLabel(c: number): string {
    if (c >= 0.66) return "Strong";
    if (c >= 0.4) return "Likely";
    return "Uncertain";
  }

  function confidenceTextClass(c: number): string {
    if (c >= 0.66) return "text-emerald-500";
    if (c >= 0.4) return "text-amber-500";
    return "text-muted-foreground";
  }

  function confidenceBarClass(c: number): string {
    if (c >= 0.66) return "bg-emerald-500";
    if (c >= 0.4) return "bg-amber-500";
    return "bg-muted-foreground/60";
  }

  function previewAt(seg: SilenceSegment) {
    store.currentTime = seg.start;
  }

  /** Apply the keep-margin and commit a cut. Returns true if a cut landed. */
  function commitCut(seg: SilenceSegment): boolean {
    const pad = Math.min(CUT_PADDING, (seg.end - seg.start) / 3);
    const start = seg.start + pad;
    const end = seg.end - pad;
    if (end - start < 0.2) return false;
    return store.addCut(start, end, "silence") !== null;
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

  // Reverts the user's accumulated "dismiss" decisions for this project so
  // previously-rejected suggestions can be re-surfaced. Re-runs detection
  // immediately so the popover repopulates without a manual click.
  function resetDismissedAndRescan() {
    store.clearDismissedSilences();
    void loadSuggestions();
  }

  const bulkCount = $derived(
    pending.filter((s) => !isBlocked(s) && s.confidence >= BULK_MIN_CONFIDENCE)
      .length,
  );
  const totalRecoverable = $derived(
    pending
      .filter((s) => !isBlocked(s))
      .reduce((sum, s) => sum + (s.end - s.start), 0),
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
                blocked ? "border-amber-500/40" : "border-border",
              )}
            >
              <VolumeX size={12} class={blocked ? "text-amber-500" : "text-primary"} />
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
                  <span class="inline-flex items-center gap-1 text-amber-500">
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
