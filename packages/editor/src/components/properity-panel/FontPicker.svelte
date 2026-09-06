<script lang="ts">
// Searchable font combobox shared by the caption and annotation panels; a Google font is fetched and registered on demand.

import { Check, ChevronsUpDown, Search } from "@recast/icons";
import * as Popover from "@recast/ui/popover";
import {
	ensureFontLoaded,
	type FontOption,
	fontLabel,
	GOOGLE_FONT_OPTIONS,
	SYSTEM_FONTS,
} from "../../lib/fonts/font-options";

let {
	value,
	weight = 400,
	onChange,
}: {
	value: string;
	/** Weight to fetch for Google fonts. */
	weight?: number;
	onChange: (value: string) => void;
} = $props();

let open = $state(false);
let query = $state("");

const filtered = $derived.by(() => {
	const q = query.trim().toLowerCase();
	const match = (f: FontOption) => !q || f.label.toLowerCase().includes(q);
	return {
		system: SYSTEM_FONTS.filter(match),
		google: GOOGLE_FONT_OPTIONS.filter(match),
	};
});

function onOpenChange(next: boolean) {
	open = next;
	if (next) query = "";
}

function pick(v: string) {
	onChange(v);
	ensureFontLoaded(v, weight);
	open = false;
}
</script>

<Popover.Root {open} {onOpenChange}>
  <Popover.Trigger>
    {#snippet child({ props })}
      <button
        {...props}
        class="flex h-7 w-36 items-center gap-1.5 rounded-md border border-border/60 bg-card/60 px-2 text-left text-[11px] transition-colors hover:border-border hover:bg-card"
      >
        <span class="min-w-0 flex-1 truncate" style="font-family: {value}">
          {fontLabel(value)}
        </span>
        <ChevronsUpDown size={12} class="shrink-0 text-muted-foreground" />
      </button>
    {/snippet}
  </Popover.Trigger>
  <Popover.Content align="end" sideOffset={6} class="w-56 p-0">
    <div class="flex items-center gap-1.5 border-b border-border/60 px-2.5">
      <Search size={13} class="shrink-0 text-muted-foreground" />
      <!-- svelte-ignore a11y_autofocus -->
      <input
        bind:value={query}
        autofocus
        placeholder="Search fonts…"
        class="h-9 w-full bg-transparent text-[12px] outline-none placeholder:text-muted-foreground"
      />
    </div>
    <div class="max-h-72 overflow-y-auto scrollbar-transparent p-1">
      {#if filtered.system.length}
        <p class="px-2 pb-1 pt-1.5 text-[9.5px] font-semibold uppercase tracking-wider text-muted-foreground">
          System
        </p>
        {#each filtered.system as f (f.value)}
          {@render row(f)}
        {/each}
      {/if}

      {#if filtered.google.length}
        <p class="px-2 pb-1 pt-2 text-[9.5px] font-semibold uppercase tracking-wider text-muted-foreground">
          Google Fonts
        </p>
        {#each filtered.google as f (f.value)}
          {@render row(f)}
        {/each}
      {/if}

      {#if !filtered.system.length && !filtered.google.length}
        <p class="py-6 text-center text-[11px] text-muted-foreground">No fonts found</p>
      {/if}
    </div>
  </Popover.Content>
</Popover.Root>

{#snippet row(f: FontOption)}
  <button
    type="button"
    onclick={() => pick(f.value)}
    class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left transition-colors hover:bg-muted/60"
  >
    <span class="flex size-4 shrink-0 items-center justify-center">
      {#if f.value === value}<Check size={13} class="text-primary" />{/if}
    </span>
    <span class="min-w-0 flex-1 truncate text-[12px]" style="font-family: {f.value}">{f.label}</span>
  </button>
{/snippet}
