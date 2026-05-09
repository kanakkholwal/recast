<script lang="ts">
  import { commandPalette } from "$lib/stores/command-palette.svelte";
  import { Search } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { Kbd } from "@recast/ui/kbd";
  import { cn } from "@recast/ui/utils";

  // Trigger-only. The dialog and the global ⌘K binding live in
  // CommandPaletteHost.svelte (mounted once at the root layout) so they
  // remain available on routes that don't include the sidebar — e.g. the
  // editor route.
  let { iconOnly } = $props<{ iconOnly?: boolean }>();
</script>

<Button
  onclick={() => commandPalette.show()}
  aria-label="Open Command Menu"
  title="Open Command Menu (⌘K)"
  variant="raw"
  size="sm"
  class={cn(
    "border border-foreground/5 group relative h-8 bg-input",
    iconOnly ? "w-8" : "min-w-8 w-full max-w-xs",
  )}
>
  <Search class="size-4 shrink-0 opacity-50 transition-opacity group-hover:opacity-70" />
  {#if !iconOnly}
    <span class="flex-1 text-left text-xs font-medium">Search…</span>
    <Kbd class="hidden sm:inline-flex">
      <span class="text-[8px] font-semibold">⌘</span>
      <span class="text-[11px]">K</span>
    </Kbd>
  {/if}
</Button>
