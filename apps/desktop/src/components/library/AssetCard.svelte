<script lang="ts">
import {
	CARD_OVERLAY_CLASS,
	cardActionsClass,
	selectTickClass,
	thumbFrameClass,
	type LibraryView,
} from "$lib/library/card-styles";
import { libraryDate } from "$lib/library/thumbnails";
import type { RecordingEntry } from "$lib/ipc";
import { Check, Film, Play, type IconComponent } from "@recast/icons";
import { Cutout } from "@recast/ui/cutout";
import { cn } from "@recast/ui/utils";
import { formatSize } from "@recast/editor/lib/format/files";
import type { Snippet } from "svelte";

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
const metaLine = $derived(meta ?? `${formatSize(entry.sizeBytes)} · ${libraryDate(entry.created)}`);
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
        class="flex size-9 items-center justify-center rounded-full bg-background/85 text-foreground shadow-craft-sm backdrop-blur"
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

<div class={cn("flex min-w-0 flex-1 flex-col gap-0.5", view === "grid" && "px-3 py-2.5")}>
  <div class="truncate text-[12.5px] font-semibold text-foreground">
    {entry.filename}
  </div>
  <div class="truncate text-[10.5px] text-muted-foreground/80">
    {metaLine}
  </div>
  {#if footer}
    {@render footer()}
  {/if}
</div>

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
