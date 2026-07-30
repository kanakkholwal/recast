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
  class="group/search flex h-12 items-center gap-3 rounded-xl border border-border/60 bg-card/70 px-4 shadow-(--shadow-craft-inset) backdrop-blur transition-all duration-200 hover:border-border hover:bg-card hover:shadow-craft-sm focus-within:border-border focus-within:bg-card focus-within:shadow-craft-sm"
>
  <Search
    class="size-4 shrink-0 text-muted-foreground/70 transition-colors group-hover/search:text-foreground group-focus-within/search:text-foreground"
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
    placeholder={`Search ${noun}…  (press / )`}
    aria-label={`Search ${noun}`}
    class="flex-1 bg-transparent text-[13px] font-medium text-foreground placeholder:text-muted-foreground/80 focus:outline-none"
  />
  {#if value}
    <Button
      variant="ghost"
      size="icon-sm"
      class="size-6"
      onclick={() => (value = "")}
      aria-label="Clear search"
      title="Clear search"
    >
      <X class="size-3" />
    </Button>
  {/if}
</label>
