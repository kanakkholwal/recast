<script lang="ts">
  import SearchCommandMenu from "$components/layout/SearchCommandMenu.svelte";
  import { Hexagon } from "@recast/icons";
  import { Trigger as SidebarTrigger } from "@recast/ui/sidebar";
</script>

<header class="flex h-16 items-center justify-between px-4">
  <a
    href="/"
    class="group flex items-center gap-3 transition-opacity hover:opacity-80"
  >
    <div class="flex h-16 shrink-0 select-none items-center gap-3 px-6">
      <div class="flex size-8 items-center justify-center rounded-xl bg-primary text-primary-foreground shadow-sm">
        <Hexagon size={18} class="fill-current" stroke={2.5} />
      </div>
      <h1 class="text-base font-semibold tracking-tight group-data-[state=collapsed]:hidden">
        Recast Studio
      </h1>
    </div>
  </a>
  <div class="inline-flex items-center gap-2">
    <SearchCommandMenu iconOnly />
    <SidebarTrigger />
  </div>
</header>
