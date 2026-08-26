<script lang="ts">
import { formatSize } from "@recast/editor/lib/format/files";
import { Check, Film, type IconComponent, Play } from "@recast/icons";
import { Cutout } from "@recast/ui/cutout";
import { cn } from "@recast/ui/utils";
import type { Snippet } from "svelte";
import type { RecordingEntry } from "$lib/ipc";
import {
	CARD_OVERLAY_CLASS,
	cardActionsClass,
	type LibraryView,
	selectTickClass,
	thumbFrameClass,
} from "$lib/library/card-styles";
import { libraryDate } from "$lib/library/thumbnails";

let {
	entry,
	thumbnail,
	view,
	selectMode = false,
	selected = false,
	onOpen,
	placeholderIcon = Film,
	typeLabel,
	meta,
	badge,
	footer,
	actions,
}: {
	entry: RecordingEntry;
	thumbnail?: string;
	view: LibraryView;
	selectMode?: boolean;
	selected?: boolean;
	onOpen: () => void;
	placeholderIcon?: IconComponent;
	typeLabel?: string;
	meta?: string;
	badge?: Snippet;
	footer?: Snippet;
	actions?: Snippet;
} = $props();

const Placeholder = $derived(placeholderIcon);
// The extension lives in the type badge, so the title drops it; the card
// shell's `title` attribute still carries the full filename.
const displayName = $derived(entry.filename.replace(/\.[^.]+$/, "") || entry.filename);
const sizeLabel = $derived(formatSize(entry.sizeBytes));
const dateLabel = $derived(libraryDate(entry.created));
</script>

<div class={thumbFrameClass(view)}>
  {#if thumbnail}
    <img
      src={thumbnail}
      alt=""
      draggable="false"
      class="size-full object-cover transition-transform duration-300 motion-safe:group-hover/card:scale-[1.03]"
    />
  {:else}
    <div class="grid size-full place-items-center text-muted-foreground/50">
      <Placeholder class={view === "grid" ? "size-6" : "size-4"} />
    </div>
  {/if}

  {#if selectMode}
    <div class="absolute left-1.5 top-1.5 z-10">
      <span class={selectTickClass(selected)}>
        {#if selected}<Check size={12} />{/if}
      </span>
    </div>
  {:else if view === "grid"}
    <div
      class="pointer-events-none absolute inset-0 grid place-items-center bg-linear-to-t from-black/40 via-transparent to-transparent opacity-0 transition-opacity duration-200 group-hover/card:opacity-100"
    >
      <span
        class="flex size-9 items-center justify-center rounded-full bg-foreground/90 text-background shadow-craft-md backdrop-blur transition-transform duration-200 motion-safe:group-hover/card:scale-105"
      >
        <Play class="size-4 translate-x-px" />
      </span>
    </div>
  {/if}

  {#if view === "grid" && (badge || typeLabel)}
    <Cutout corner="bl" surface="card" radius={8} class="flex items-center px-2.5 pt-2.5 pb-1">
      {#if badge}
        {@render badge()}
      {:else}
        <span
          class="text-[8.5px] font-bold uppercase leading-none tracking-wider text-muted-foreground"
        >
          {typeLabel}
        </span>
      {/if}
    </Cutout>
  {/if}
</div>

{#if view === "grid"}
  <div class="flex min-w-0 flex-1 flex-col gap-1 px-3.5 py-3">
    <div class="truncate text-[13px] font-semibold tracking-tight text-foreground">
      {displayName}
    </div>
    <div class="truncate text-[10.5px] tabular-nums text-muted-foreground">
      {meta ?? `${sizeLabel} · ${dateLabel}`}
    </div>
    {#if footer}
      {@render footer()}
    {/if}
  </div>
{:else}
  <!-- List row: name + type chip up top, size/date pushed right as a quiet
       tabular column so rows scan like a table without drawing one. -->
  <div class="flex min-w-0 flex-1 items-center gap-3 pr-2">
    <div class="min-w-0 flex-1">
      <div class="flex min-w-0 items-center gap-1.5">
        <span class="truncate text-[12.5px] font-semibold tracking-tight text-foreground">
          {displayName}
        </span>
        {#if typeLabel}
          <span
            class="shrink-0 rounded bg-muted/60 px-1 py-0.5 text-[8.5px] font-bold uppercase leading-none tracking-wider text-muted-foreground ring-1 ring-inset ring-border/40"
          >
            {typeLabel}
          </span>
        {/if}
      </div>
      {#if footer}
        {@render footer()}
      {/if}
    </div>
    <div
      class="hidden shrink-0 items-center gap-3 text-[11px] tabular-nums text-muted-foreground sm:flex"
    >
      <span class="w-16 text-right">{meta ?? sizeLabel}</span>
      {#if !meta}
        <span class="w-24 truncate text-right text-muted-foreground/80">{dateLabel}</span>
      {/if}
    </div>
  </div>
{/if}

<button
  type="button"
  onclick={onOpen}
  aria-pressed={selectMode ? selected : undefined}
  class={CARD_OVERLAY_CLASS}
>
  <span class="sr-only">
    {selectMode ? `Select ${entry.filename}` : `Open ${entry.filename}`}
  </span>
</button>

{#if !selectMode && actions}
  <div class={cardActionsClass(view)}>
    {@render actions()}
  </div>
{/if}
