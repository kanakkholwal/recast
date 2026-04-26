<script lang="ts">
  import { suggestZoomRegions, type ZoomSuggestion } from "$lib/ipc";
  import type { EditorStore } from "$lib/stores/editor-store.svelte";
  import {
    findFreeSlot as _findFreeSlot,
    planPlacement,
    type Interval,
  } from "$lib/zoom/auto-apply";
  import { AlertTriangle, Check, MousePointerClick, Sparkles, Wand2, XCircle } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import * as Tooltip from "@recast/ui/tooltip";
  import { cn } from "@recast/ui/utils";

  interface Props {
    store: EditorStore;
    onclose: () => void;
  }

  let { store, onclose }: Props = $props();
  // Re-export to silence unused-import noise in case findFreeSlot is removed below.
  void _findFreeSlot;

  type Status = "idle" | "loading" | "ready" | "error" | "empty";
  let status = $state<Status>("idle");
  let errorMsg = $state<string | null>(null);
  // Suggestions the user hasn't yet accepted or dismissed. Each refresh
  // discards the previous set so we don't keep stale suggestions around.
  let pending = $state<ZoomSuggestion[]>([]);

  $effect(() => {
    void loadSuggestions();
  });

  async function loadSuggestions() {
    if (!store.cursorPath) {
      status = "error";
      errorMsg = "This clip has no captured cursor data to analyse.";
      return;
    }
    status = "loading";
    errorMsg = null;
    try {
      const result = await suggestZoomRegions(store.cursorPath);
      pending = result;
      status = result.length === 0 ? "empty" : "ready";
    } catch (err) {
      console.error("Failed to load zoom suggestions", err);
      errorMsg = err instanceof Error ? err.message : String(err);
      status = "error";
    }
  }

  function formatTime(us: number): string {
    const s = us / 1_000_000;
    const m = Math.floor(s / 60);
    const rem = s - m * 60;
    return `${m}:${rem.toFixed(2).padStart(5, "0")}`;
  }

  function reasonLabel(r: ZoomSuggestion["reason"]): string {
    return r === "click" ? "Click" : "Settle";
  }

  function reasonIcon(r: ZoomSuggestion["reason"]) {
    return r === "click" ? MousePointerClick : Sparkles;
  }

  function previewAt(sug: ZoomSuggestion) {
    store.currentTime = sug.timestampUs / 1_000_000;
  }

  function currentOccupied(): Interval[] {
    return store.zoomRegions
      .map((z) => ({ start: z.start, end: z.end }))
      .sort((a, b) => a.start - b.start);
  }

  function clipBounds(): { start: number; end: number } | null {
    const duration = store.metadata?.duration ?? 0;
    if (duration <= 0) return null;
    return { start: store.trimStart, end: store.trimEnd || duration };
  }

  // Derive per-suggestion placement (or null) so blocked rows can be greyed
  // out before the user tries to accept them. Re-plans whenever zoom regions
  // or trim change.
  const placements = $derived.by(() => {
    const bounds = clipBounds();
    if (!bounds) return new Map<string, Interval | null>();
    const occupied = currentOccupied();
    const map = new Map<string, Interval | null>();
    for (const sug of pending) {
      const centerSec = sug.timestampUs / 1_000_000;
      const key = sug.timestampUs + "-" + sug.reason;
      map.set(key, planPlacement(occupied, bounds.start, bounds.end, centerSec));
    }
    return map;
  });

  function keyOf(sug: ZoomSuggestion) {
    return sug.timestampUs + "-" + sug.reason;
  }

  function accept(idx: number) {
    const sug = pending[idx];
    if (!sug) return;
    const bounds = clipBounds();
    if (!bounds) return;
    const plan = planPlacement(currentOccupied(), bounds.start, bounds.end, sug.timestampUs / 1_000_000);
    if (!plan) return; // blocked — button should already be disabled
    store.addZoomRegion(plan.start, plan.end, 1.8, centerOf(sug));
    pending = pending.filter((_, i) => i !== idx);
    if (pending.length === 0) status = "empty";
  }

  function centerOf(sug: ZoomSuggestion): { x: number; y: number } | undefined {
    const w = store.metadata?.width ?? 0;
    const h = store.metadata?.height ?? 0;
    if (w <= 0 || h <= 0) return undefined;
    return {
      x: Math.min(1, Math.max(0, sug.x / w)),
      y: Math.min(1, Math.max(0, sug.y / h)),
    };
  }

  function dismiss(idx: number) {
    pending = pending.filter((_, i) => i !== idx);
    if (pending.length === 0) status = "empty";
  }

  function acceptAll() {
    const bounds = clipBounds();
    if (!bounds) return;
    // Re-plan after each placement so two adjacent suggestions don't both claim
    // the same slot — a click + settle 400 ms apart would otherwise produce
    // overlapping windows. We sort by timestamp so earlier triggers win.
    const occupied = currentOccupied();
    const sorted = [...pending].sort((a, b) => a.timestampUs - b.timestampUs);
    const skipped: ZoomSuggestion[] = [];
    for (const sug of sorted) {
      const plan = planPlacement(occupied, bounds.start, bounds.end, sug.timestampUs / 1_000_000);
      if (!plan) {
        skipped.push(sug);
        continue;
      }
      store.addZoomRegion(plan.start, plan.end, 1.8, centerOf(sug));
      occupied.push(plan);
      occupied.sort((a, b) => a.start - b.start);
    }
    pending = skipped;
    if (pending.length === 0) status = "empty";
  }

  function dismissAll() {
    pending = [];
    status = "empty";
  }
</script>

<div
  role="dialog"
  aria-label="Auto-focus suggestions"
  class="flex max-h-[60vh] w-80 flex-col overflow-hidden rounded-md border border-border bg-popover text-popover-foreground shadow-xl ring-1 ring-border"
>
  <header class="flex items-center justify-between gap-2 border-b border-border px-3 py-2">
    <div class="flex items-center gap-1.5">
      <Wand2 size={13} class="text-primary" />
      <h3 class="text-[11px] font-semibold tracking-tight">Auto-focus</h3>
    </div>
    <Button variant="ghost" size="xs" onclick={onclose} class="gap-1.5">
      <XCircle size={11} />
      Close
    </Button>
  </header>

  {#if status === "loading"}
    <div class="flex items-center justify-center gap-2 px-3 py-6 text-[11px] text-muted-foreground">
      <div class="size-3 animate-spin rounded-full border border-muted-foreground/40 border-t-foreground"></div>
      Analysing cursor activity…
    </div>
  {:else if status === "error"}
    <div class="flex flex-col gap-2 px-3 py-3 text-[11px] text-muted-foreground">
      <p>{errorMsg ?? "Could not load suggestions."}</p>
      <Button variant="secondary" size="xs" onclick={loadSuggestions}>Retry</Button>
    </div>
  {:else if status === "empty"}
    <div class="flex flex-col items-center gap-1 px-3 py-6 text-center text-[11px] text-muted-foreground">
      <Sparkles size={14} class="text-muted-foreground/70" />
      <p class="font-medium text-foreground">No candidates left</p>
      <p>Add a focus manually or re-run analysis.</p>
      <Button variant="ghost" size="xs" onclick={loadSuggestions} class="mt-1">Re-scan</Button>
    </div>
  {:else if status === "ready"}
    {@const availableCount = pending.filter((s) => placements.get(keyOf(s)) != null).length}
    <div class="flex items-center justify-between gap-2 border-b border-border px-3 py-1.5 text-[10px] text-muted-foreground">
      <span>
        {availableCount} of {pending.length} available
      </span>
      <div class="flex items-center gap-1">
        <Button variant="ghost" size="xs" onclick={dismissAll}>Dismiss all</Button>
        <Button
          variant="default"
          size="xs"
          class="gap-1.5"
          onclick={acceptAll}
          disabled={availableCount === 0}
        >
          <Check size={11} />
          Accept all
        </Button>
      </div>
    </div>
    <ul class="flex-1 overflow-y-auto">
      {#each pending as sug, i (keyOf(sug))}
        {@const ReasonIcon = reasonIcon(sug.reason)}
        {@const plan = placements.get(keyOf(sug))}
        {@const blocked = plan == null}
        <li>
          <button
            type="button"
            onpointerenter={() => previewAt(sug)}
            onfocus={() => previewAt(sug)}
            class={cn(
              "group flex w-full items-center gap-2 border-b border-border px-3 py-2 text-left transition-colors",
              "hover:bg-muted/50 focus-visible:bg-muted/50 focus:outline-none",
              blocked && "opacity-60",
            )}
          >
            <span
              class={cn(
                "flex size-7 shrink-0 items-center justify-center rounded-md border bg-card",
                blocked ? "border-amber-500/40" : "border-border",
              )}
            >
              <ReasonIcon size={12} class={blocked ? "text-amber-500" : "text-primary"} />
            </span>
            <div class="flex-1 min-w-0">
              <div class="flex items-baseline justify-between gap-2">
                <span class="truncate text-[11px] font-medium text-foreground">
                  {reasonLabel(sug.reason)}
                </span>
                <span class="font-mono text-[10px] tabular-nums text-muted-foreground">
                  {formatTime(sug.timestampUs)}
                </span>
              </div>
              <div class="truncate text-[10px] text-muted-foreground">
                {#if blocked}
                  <span class="inline-flex items-center gap-1 text-amber-500">
                    <AlertTriangle size={9} />
                    Overlaps an existing focus
                  </span>
                {:else}
                  x {sug.x}, y {sug.y}
                {/if}
              </div>
            </div>
            <div class="flex shrink-0 items-center gap-1">
              <Button
                variant="ghost"
                size="xs"
                aria-label="Dismiss"
                onclick={(event) => {
                  event.stopPropagation();
                  dismiss(i);
                }}
              >
                <XCircle size={11} />
              </Button>
              {#if blocked}
                <Tooltip.Root>
                  <Tooltip.Trigger>
                    <Button variant="default" size="xs" class="gap-1" disabled aria-label="Add focus (blocked)">
                      <Check size={11} />
                    </Button>
                  </Tooltip.Trigger>
                  <Tooltip.Content>Remove the overlapping focus first</Tooltip.Content>
                </Tooltip.Root>
              {:else}
                <Button
                  variant="default"
                  size="xs"
                  class="gap-1"
                  aria-label="Add focus"
                  onclick={(event) => {
                    event.stopPropagation();
                    accept(i);
                  }}
                >
                  <Check size={11} />
                </Button>
              {/if}
            </div>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>
