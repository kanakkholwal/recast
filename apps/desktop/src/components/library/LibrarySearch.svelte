<script lang="ts">
// Search is the spine of a library page, so it owns its own shortcuts:
// `/` focuses it from anywhere on the page, Escape empties it without moving
// focus away. The page just says what is being searched.
import { Search, X } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { onMount } from "svelte";

interface Props {
	value: string;
	/** Noun for the placeholder and label, e.g. "recordings". */
	noun: string;
}

let { value = $bindable(""), noun }: Props = $props();

let el = $state<HTMLInputElement | null>(null);

onMount(() => {
	const onKey = (e: KeyboardEvent) => {
		const t = e.target as HTMLElement | null;
		const typing =
			!!t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable);
		if (e.key === "/" && !typing && !e.metaKey && !e.ctrlKey && !e.altKey) {
			e.preventDefault();
			el?.focus();
			el?.select();
		}
	};
	window.addEventListener("keydown", onKey);
	return () => window.removeEventListener("keydown", onKey);
});
</script>

<label
  class="group/search flex h-9 items-center gap-2 rounded-lg bg-muted/50 px-3 ring-1 ring-inset ring-border/40 transition-[background-color,box-shadow] duration-150 hover:bg-muted/70 focus-within:bg-card focus-within:ring-ring/50"
>
  <Search
    class="size-3.5 shrink-0 text-muted-foreground/70 transition-colors group-hover/search:text-foreground group-focus-within/search:text-foreground"
  />
  <input
    bind:this={el}
    bind:value
    onkeydown={(e) => {
      if (e.key === "Escape" && value) {
        e.preventDefault();
        value = "";
      }
    }}
    type="text"
    placeholder={`Search ${noun}…`}
    aria-label={`Search ${noun}`}
    class="flex-1 bg-transparent text-[12.5px] font-medium text-foreground placeholder:text-muted-foreground/70 focus:outline-none"
  />
  {#if !value}
    <kbd
      class="pointer-events-none hidden rounded border border-border/50 bg-background/60 px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground/70 group-focus-within/search:hidden sm:block"
      aria-hidden="true"
    >
      /
    </kbd>
  {/if}
  {#if value}
    <Button
      variant="ghost"
      size="icon-sm"
      class="-mr-1 size-5"
      onclick={() => (value = "")}
      aria-label="Clear search"
      title="Clear search"
    >
      <X class="size-3" />
    </Button>
  {/if}
</label>
