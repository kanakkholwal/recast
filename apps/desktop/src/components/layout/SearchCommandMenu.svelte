<script lang="ts">
  import { commandPalette } from "$lib/stores/command-palette.svelte";
  import { Search } from "@recast/icons";
  import { Button } from "@recast/ui/button";
  import { Kbd } from "@recast/ui/kbd";
  import { cn } from "@recast/ui/utils";

  // Trigger-only; the dialog and ⌘K binding live in CommandPaletteHost
  // (mounted at the root layout, so it works on sidebar-less routes).
  let { iconOnly } = $props<{ iconOnly?: boolean }>();
</script>


<Button
  onclick={() => commandPalette.show()}
  aria-label="Open Command Menu"
  title="Open Command Menu (⌘K)"
  variant="raw"
  size="sm"
  class={cn(
    "group relative flex h-8 w-full items-center justify-start gap-2 overflow-hidden rounded-lg border border-foreground/5 bg-card/80 px-2.5 transition-colors duration-200",
  )}
>
  <!-- `justify-start` + symmetric `px-2.5` around a 14px icon keeps it exactly
       centered in the collapsed 34px rail (10 + 14 + 10), matching the nav
       rows. The Kbd's `ml-auto` only pushes the (collapsing) shortcut right; it
       never moves the icon. -->
  <Search class="size-3.5 shrink-0 opacity-50 transition-opacity group-hover:opacity-70" />
  <span
    class={cn(
      "min-w-0 truncate text-left text-xs font-medium text-muted-foreground transition-[max-width,opacity] duration-200 ease-linear",
      iconOnly ? "max-w-0 opacity-0" : "max-w-40 opacity-100",
    )}
  >
    Search…
  </span>
  <Kbd
    class={cn(
      "ml-auto hidden shrink-0 transition-[max-width,opacity] duration-200 ease-linear sm:inline-flex",
      iconOnly ? "max-w-0 opacity-0" : "max-w-16 opacity-100",
    )}
  >
    <span class="text-[8px] font-semibold">⌘</span>
    <span class="text-[10px]">K</span>
  </Kbd>
</Button>
