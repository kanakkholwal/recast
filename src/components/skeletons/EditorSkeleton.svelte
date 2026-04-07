<script lang="ts">
  import { Skeleton } from "$components/ui/skeleton";
</script>

<div class="flex flex-1 min-h-0">
  <!-- Left column: preview + controls + timeline -->
  <div class="flex-1 flex flex-col min-h-0">
    <!-- Sub-toolbar -->
    <div class="h-9 flex items-center gap-3 px-4 border-b border-border">
      <Skeleton class="h-6 w-24 rounded-md" />
      <Skeleton class="h-6 w-16 rounded-md" />
      <div class="ml-auto">
        <Skeleton class="h-6 w-20 rounded-md" />
      </div>
    </div>

    <!-- Preview area -->
    <div class="flex-1 flex items-center justify-center p-6">
      <Skeleton class="w-full max-w-[900px] aspect-video rounded-[24px]" />
    </div>

    <!-- Playback controls -->
    <div class="shrink-0 px-4 py-2 border-t border-border">
      <div class="flex items-center justify-between">
        <Skeleton class="h-5 w-32" />
        <div class="flex items-center gap-2">
          <Skeleton class="h-8 w-8 rounded-full" />
          <Skeleton class="h-10 w-10 rounded-full" />
          <Skeleton class="h-8 w-8 rounded-full" />
        </div>
        <Skeleton class="h-5 w-24" />
      </div>
    </div>

    <!-- Timeline -->
    <div class="shrink-0 px-4 pb-3 pt-2 border-t border-border">
      <div class="flex items-center gap-2 mb-2">
        <Skeleton class="h-6 w-6 rounded" />
        <Skeleton class="h-6 w-6 rounded" />
        <Skeleton class="h-6 w-6 rounded" />
        <div class="ml-auto">
          <Skeleton class="h-4 w-20" />
        </div>
      </div>
      <Skeleton class="h-16 w-full rounded-[16px]" />
    </div>
  </div>

  <!-- Right column: properties panel -->
  <div class="w-85 shrink-0 border-l border-border">
    <div class="p-4 border-b border-border space-y-3">
      <Skeleton class="h-5 w-24" />
      <div class="flex gap-2">
        <Skeleton class="h-5 w-16 rounded-full" />
        <Skeleton class="h-5 w-20 rounded-full" />
        <Skeleton class="h-5 w-14 rounded-full" />
      </div>
    </div>
    <div class="flex gap-1 p-2 border-b border-border">
      {#each Array(4) as _}
        <Skeleton class="h-8 w-8 rounded-md" />
      {/each}
    </div>
    <div class="p-4 space-y-4">
      {#each Array(5) as _}
        <div class="space-y-2">
          <Skeleton class="h-4 w-28" />
          <Skeleton class="h-8 w-full rounded-md" />
        </div>
      {/each}
    </div>
  </div>
</div>
