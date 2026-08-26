<script lang="ts">
// Inspector for one screen state: the frame that was read, every element the OCR
// engine found drawn in place over it, and the same elements as a readable list.
// This is the surface that answers "why did it produce THAT?", so the picture and
// the structured output have to be visibly the same thing: a box on the frame and
// its row in the list are one element, and highlighting either highlights both.

import { Check, Copy, SquareDashedMousePointer } from "@recast/icons";
import { Badge } from "@recast/ui/badge";
import { Button } from "@recast/ui/button";
import { SegmentedToggle } from "@recast/ui/segmented";
import { clock } from "../../lib/format/time";
import type { ScreenStateSpan } from "../../lib/wire-types";
import DialogShell from "../dialog/DialogShell.svelte";
import { boxLabel, boxStyle, regionLabel, spanAsText } from "./dev-ocr-panel.logic";

interface Props {
	span: ScreenStateSpan | null;
	open: boolean;
	onOpenChange: (open: boolean) => void;
	onSeek: (t: number) => void;
}
let { span, open, onOpenChange, onSeek }: Props = $props();

// Two ways to read the same frame: the raw capture, and the capture with every
// recognized box drawn on it. Toggling between them is how you check the OCR
// against what was actually on screen. Defaults to annotated (the reason to open).
let annotated = $state(true);

// Hover/focus is a transient preview; a click pins the element so it survives the
// pointer moving away. Highlighting either the box or the row lights up both.
let previewed = $state<number | null>(null);
let pinned = $state<number | null>(null);
const active = $derived(previewed ?? pinned);

let rowEls: Record<number, HTMLElement | undefined> = {};
let copied = $state(false);

function pin(id: number) {
	pinned = id;
	rowEls[id]?.scrollIntoView({ block: "nearest", behavior: "smooth" });
}

async function copyAsText() {
	if (!span) return;
	await navigator.clipboard.writeText(spanAsText(span, clock));
	copied = true;
	setTimeout(() => (copied = false), 1500);
}

function jump() {
	if (!span) return;
	onSeek(span.start);
	onOpenChange(false);
}
</script>

{#if span}
  <DialogShell
    {open}
    title={`Screen at ${clock(span.start)}`}
    subtitle={`Held until ${clock(span.end)} · ${span.elements.length} ${span.elements.length === 1 ? "element" : "elements"} read`}
    icon={SquareDashedMousePointer}
    widthClass="sm:max-w-4xl"
    {onOpenChange}
  >

      <div class="grid gap-4 md:grid-cols-[1.6fr_1fr]">
        <div class="flex flex-col gap-2 self-start">
          <SegmentedToggle
            checked={annotated}
            onCheckedChange={(v) => (annotated = v)}
            offLabel="Original"
            onLabel="Annotated"
            size="xs"
            fill
            aria-label="Toggle the OCR overlay on the frame"
          />

          <!-- The frame. In "Annotated" every recognized box is drawn where it was
               found; in "Original" it is the raw capture, to check the read against. -->
          <div class="border-border bg-muted relative overflow-hidden rounded-lg border">
            {#if span.preview}
              <img src={span.preview} alt="Frame read at {clock(span.start)}" class="block w-full" />
            {:else}
              <div class="text-muted-foreground flex aspect-video items-center justify-center text-xs">
                No preview was captured for this frame.
              </div>
            {/if}

            {#if annotated}
              {#each span.elements as el (el.id)}
                <button
                  type="button"
                  class="focus-visible:ring-ring absolute rounded-[2px] border transition-colors focus-visible:ring-2 focus-visible:outline-none {active ===
                  el.id
                    ? 'border-primary bg-primary/25'
                    : 'border-primary/70 bg-primary/10'}"
                  style={boxStyle(el.bbox)}
                  aria-label="Element {el.id}: {el.content}"
                  onclick={() => pin(el.id)}
                  onmouseenter={() => (previewed = el.id)}
                  onmouseleave={() => (previewed = null)}
                  onfocus={() => (previewed = el.id)}
                  onblur={() => (previewed = null)}
                >
                  <span
                    class="bg-primary text-primary-foreground absolute -top-px -left-px rounded-[2px] px-1 text-[9px] leading-[13px] font-medium tabular-nums"
                  >
                    {el.id}
                  </span>
                </button>
              {/each}
            {/if}
          </div>
        </div>

        <!-- The same elements, as prose. Numbered to match the boxes on the frame. -->
        <div class="flex max-h-[60vh] flex-col gap-1.5 overflow-y-auto pr-1">
          {#each span.elements as el (el.id)}
            <button
              type="button"
              bind:this={rowEls[el.id]}
              class="focus-visible:ring-ring rounded-md border p-2 text-left transition-colors focus-visible:ring-2 focus-visible:outline-none {active ===
              el.id
                ? 'border-primary bg-accent/50'
                : 'border-border'}"
              onclick={() => (pinned = el.id)}
              onmouseenter={() => (previewed = el.id)}
              onmouseleave={() => (previewed = null)}
              onfocus={() => (previewed = el.id)}
              onblur={() => (previewed = null)}
            >
              <span class="flex items-center gap-1.5">
                <span
                  class="bg-primary text-primary-foreground flex size-4 shrink-0 items-center justify-center rounded-[3px] text-[9px] font-medium tabular-nums"
                >
                  {el.id}
                </span>
                <Badge variant="secondary" class="h-4 px-1 text-[9px] capitalize">{el.kind}</Badge>
                <span class="text-muted-foreground truncate text-[10px]">{regionLabel(el.bbox)}</span>
              </span>
              <span class="mt-1 block text-xs leading-snug break-words">{el.content}</span>
              <span class="text-muted-foreground mt-0.5 block text-[10px] tabular-nums">
                {boxLabel(el.bbox)} · read by {el.source}
              </span>
            </button>
          {:else}
            <p class="text-muted-foreground text-xs">
              No text was recognized in this frame. It was kept because the picture
              changed, which is worth knowing on its own.
            </p>
          {/each}
        </div>
      </div>

      {#snippet footer()}
        <Button variant="ghost" size="xs" class="mr-auto" onclick={copyAsText}>
          {#if copied}
            <Check class="size-3.5" />
            Copied
          {:else}
            <Copy class="size-3.5" />
            Copy as text
          {/if}
        </Button>
        <Button size="xs" onclick={jump}>Jump to {clock(span.start)}</Button>
      {/snippet}
  </DialogShell>
{/if}
