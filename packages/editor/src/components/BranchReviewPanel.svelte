<script lang="ts">
import { Check, Eye, GitGraph, LoaderCircle, Trash2, TriangleAlert, Undo2 } from "@recast/icons";
import { Badge } from "@recast/ui/badge";
import { Button } from "@recast/ui/button";
import * as Popover from "@recast/ui/popover";
import { prefersReducedMotion } from "svelte/motion";
import { scale } from "svelte/transition";
import { branchReview } from "../lib/agent/branch-store.svelte";
import type { EditorRenderState } from "../lib/editor/render-state";
import { relativeAge, summariseChanges, toSections } from "./branch-review.logic";

interface Props {
	projectPath: string;
	/** Identifies this GUI as the writer when a branch is applied. */
	writerId: string;
	/** Load a branch's state into the editor read-only, for preview. */
	onPreview?: (state: Partial<EditorRenderState>) => void;
	/** Refresh the editor after a branch lands. */
	onApplied?: () => void;
}

let { projectPath, writerId, onPreview, onApplied }: Props = $props();

let open = $state(false);
// Sampled once per open: a live clock would rerender the list every second for
// a label that only reads "5m ago".
let openedAtMs = $state(Date.now());
// Discarding deletes an agent's proposed work outright, so it asks once. Inline
// rather than a dialog: a modal over this popover would hide the list behind it.
let pendingDiscard = $state<string | null>(null);
// Preview swaps the branch into the editor. The host stacks an undo state first,
// which is worth saying rather than leaving the reader to find the way back.
let previewedId = $state<string | null>(null);

// The badge appears the moment an agent proposes something, mid-edit. Fading it
// in reads as arriving; popping in reads as a glitch.
const entrance = $derived(
	prefersReducedMotion.current ? { duration: 0 } : { duration: 180, start: 0.95, opacity: 0 },
);

$effect(() => branchReview.bind(projectPath));

const sections = $derived(toSections(branchReview.changes));
const summary = $derived(summariseChanges(branchReview.changes));

function onOpenChange(next: boolean) {
	open = next;
	pendingDiscard = null;
	if (next) {
		openedAtMs = Date.now();
		void branchReview.refresh();
	}
}

function select(id: string | null) {
	pendingDiscard = null;
	void branchReview.select(id);
}

async function preview(id: string) {
	const state = await branchReview.preview(id);
	if (!state) return;
	onPreview?.(state);
	previewedId = id;
}

async function discard(id: string) {
	if (pendingDiscard !== id) {
		pendingDiscard = id;
		return;
	}
	pendingDiscard = null;
	if (previewedId === id) previewedId = null;
	await branchReview.discard(id);
}

async function apply(id: string) {
	const report = await branchReview.apply(id, writerId);
	if (!report) return;
	previewedId = null;
	onApplied?.();
	open = false;
}
</script>

{#if branchReview.available && branchReview.count > 0}
  <Popover.Root {open} {onOpenChange}>
    <Popover.Trigger>
      {#snippet child({ props })}
        <button
          {...props as Record<string, unknown>}
          type="button"
          in:scale={entrance}
          class="flex items-center gap-1.5 rounded-full border bg-muted px-2.5 py-1 text-[11px] font-medium text-muted-foreground transition-colors hover:bg-muted/70 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
          aria-label="{branchReview.count} proposed change set{branchReview.count === 1 ? '' : 's'} to review"
        >
          <GitGraph class="size-3" aria-hidden="true" />
          {branchReview.count} proposed
        </button>
      {/snippet}
    </Popover.Trigger>

    <Popover.Content align="end" sideOffset={6} class="w-96 p-0">
      <div class="border-b px-3 py-2">
        <p class="text-[12px] font-medium">Proposed changes</p>
        <p class="text-[11px] text-muted-foreground">
          Nothing here has touched your project yet.
        </p>
      </div>

      {#if branchReview.error}
        <div
          role="alert"
          class="flex items-start gap-2 border-b bg-destructive/10 px-3 py-2 text-[11px]"
        >
          <TriangleAlert class="mt-0.5 size-3 shrink-0" aria-hidden="true" />
          <span class="flex-1">{branchReview.error}</span>
          <button
            type="button"
            class="shrink-0 underline underline-offset-2"
            onclick={() => branchReview.dismissError()}
          >
            Dismiss
          </button>
        </div>
      {/if}

      <ul class="max-h-40 overflow-y-auto border-b">
        {#each branchReview.branches as branch (branch.id)}
          {@const active = branch.id === branchReview.selectedId}
          <li>
            <button
              type="button"
              class="flex w-full flex-col items-start gap-0.5 px-3 py-2 text-left text-[11px] transition-colors hover:bg-muted/60 focus-visible:outline-none focus-visible:bg-muted/60"
              class:bg-muted={active}
              aria-current={active ? "true" : undefined}
              onclick={() => select(active ? null : branch.id)}
            >
              <span class="flex w-full items-baseline justify-between gap-2">
                <span class="font-medium">{branch.label || branch.id}</span>
                <span class="shrink-0 tabular-nums text-muted-foreground">
                  {relativeAge(branch.updatedAtMs, openedAtMs)}
                </span>
              </span>
              <span class="text-muted-foreground">
                {branch.author} · {branch.ops} op{branch.ops === 1 ? "" : "s"}
                {#if branch.stale}
                  <Badge variant="outline" class="ml-1 h-4 px-1.5 text-[10px]">stale</Badge>
                {/if}
              </span>
            </button>
          </li>
        {/each}
      </ul>

      {#if branchReview.selected}
        {@const selected = branchReview.selected}
        <div class="border-b px-3 py-1.5 text-[11px] text-muted-foreground">{summary}</div>

        <div class="max-h-56 overflow-y-auto px-3 py-2">
          {#if branchReview.loading}
            <p class="flex items-center gap-1.5 py-2 text-[11px] text-muted-foreground">
              <LoaderCircle class="size-3 animate-spin motion-reduce:animate-none" aria-hidden="true" />
              Loading changes…
            </p>
          {:else if sections.length === 0}
            <p class="py-2 text-[11px] text-muted-foreground">This branch changes nothing.</p>
          {:else}
            {#each sections as section (section.group)}
              <div class="mb-2 last:mb-0">
                <p class="mb-1 text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
                  {section.label}
                </p>
                <ul class="flex flex-col gap-1">
                  {#each section.rows as row (row.field)}
                    <li class="flex items-baseline justify-between gap-2 text-[11px]">
                      <span class="truncate">{row.label}</span>
                      <span class="shrink-0 tabular-nums text-muted-foreground">
                        {#if row.kind === "added"}
                          {row.after}
                        {:else if row.kind === "removed"}
                          <s>{row.before}</s>
                        {:else}
                          <s>{row.before}</s> → {row.after}
                        {/if}
                      </span>
                    </li>
                  {/each}
                </ul>
              </div>
            {/each}
          {/if}
        </div>

        {#if previewedId === selected.id}
          <p class="px-3 pb-1 text-[11px] text-muted-foreground">
            Previewing in the editor. Undo puts your own edit back.
          </p>
        {/if}

        <!-- The branch-shaping actions group left; Apply owns the right edge
             alone, so the button next to Discard is never the one that writes. -->
        <div class="flex items-center gap-1.5 px-3 py-2">
          <Button
            variant="ghost"
            size="sm"
            class="h-7 px-2 text-[11px]"
            disabled={branchReview.busy}
            onclick={() => preview(selected.id)}
          >
            <Eye class="size-3" aria-hidden="true" />
            Preview
          </Button>
          {#if selected.seq > 1}
            <Button
              variant="ghost"
              size="sm"
              class="h-7 px-2 text-[11px]"
              disabled={branchReview.busy}
              onclick={() => branchReview.truncate(selected.id, selected.seq - 1)}
            >
              <Undo2 class="size-3" aria-hidden="true" />
              Undo last
            </Button>
          {/if}
          <Button
            variant="ghost"
            size="sm"
            class="h-7 px-2 text-[11px] {pendingDiscard === selected.id
              ? 'text-destructive hover:text-destructive'
              : ''}"
            disabled={branchReview.busy}
            onclick={() => discard(selected.id)}
          >
            <Trash2 class="size-3" aria-hidden="true" />
            {pendingDiscard === selected.id ? "Confirm discard" : "Discard"}
          </Button>
          <Button
            size="sm"
            class="ml-auto h-7 px-2 text-[11px]"
            disabled={branchReview.busy || branchReview.changes.length === 0}
            onclick={() => apply(selected.id)}
          >
            <Check class="size-3" aria-hidden="true" />
            Apply
          </Button>
        </div>
      {:else}
        <p class="px-3 py-3 text-[11px] text-muted-foreground">
          Select a change set to see what it would do.
        </p>
      {/if}
    </Popover.Content>
  </Popover.Root>
{/if}
