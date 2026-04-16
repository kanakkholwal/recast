<script lang="ts">
  import { suggestZoomRegions, type ZoomSuggestion } from "$lib/ipc";
  import type { EditorStore } from "$lib/stores/editor-store.svelte";
  import { Check, MousePointerClick, Sparkles, Wand2, XCircle } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { cn } from "@recast/ui/utils";

  interface Props {
    store: EditorStore;
    onclose: () => void;
  }

  let { store, onclose }: Props = $props();

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

  function makeRegion(sug: ZoomSuggestion) {
    const duration = store.metadata?.duration ?? 0;
    if (duration <= 0) return;
    const centerSec = sug.timestampUs / 1_000_000;
    const clipEnd = store.trimEnd || duration;
    const start = Math.max(store.trimStart, centerSec - 0.5);
    const end = Math.min(clipEnd, Math.max(start + 1.0, centerSec + 1.0));
    store.addZoomRegion(start, end, 1.8);
  }

  function accept(idx: number) {
    const sug = pending[idx];
    if (!sug) return;
    makeRegion(sug);
    pending = pending.filter((_, i) => i !== idx);
    if (pending.length === 0) {
      status = "empty";
    }
  }

  function dismiss(idx: number) {
    pending = pending.filter((_, i) => i !== idx);
    if (pending.length === 0) status = "empty";
  }

  function acceptAll() {
    for (const sug of pending) makeRegion(sug);
    pending = [];
    status = "empty";
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
    <div class="flex items-center justify-between gap-2 border-b border-border px-3 py-1.5 text-[10px] text-muted-foreground">
      <span>
        {pending.length} candidate{pending.length === 1 ? "" : "s"}
      </span>
      <div class="flex items-center gap-1">
        <Button variant="ghost" size="xs" onclick={dismissAll}>Dismiss all</Button>
        <Button variant="default" size="xs" class="gap-1.5" onclick={acceptAll}>
          <Check size={11} />
          Accept all
        </Button>
      </div>
    </div>
    <ul class="flex-1 overflow-y-auto">
      {#each pending as sug, i (sug.timestampUs + "-" + sug.reason)}
        {@const ReasonIcon = reasonIcon(sug.reason)}
        <li>
          <button
            type="button"
            onpointerenter={() => previewAt(sug)}
            onfocus={() => previewAt(sug)}
            class={cn(
              "group flex w-full items-center gap-2 border-b border-border px-3 py-2 text-left transition-colors",
              "hover:bg-muted/50 focus-visible:bg-muted/50 focus:outline-none",
            )}
          >
            <span class="flex size-7 shrink-0 items-center justify-center rounded-md border border-border bg-card">
              <ReasonIcon size={12} class="text-primary" />
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
                x {sug.x}, y {sug.y}
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
            </div>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>
