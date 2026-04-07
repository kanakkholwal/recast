<script lang="ts">
  import { Skeleton } from "$components/ui/skeleton";
</script>

<div class="grid grid-cols-1 gap-6 p-8 lg:grid-cols-2 2xl:grid-cols-3">
  {#each Array(6) as _, i}
    <div
      class="rounded-xl border border-border bg-card overflow-hidden"
      style="animation-delay: {i * 60}ms"
    >
      <Skeleton class="aspect-video w-full rounded-none" />
      <div class="p-4 space-y-3">
        <div class="flex items-center gap-2">
          <Skeleton class="h-5 w-16 rounded-full" />
          <Skeleton class="h-4 w-24" />
        </div>
        <Skeleton class="h-4 w-3/4" />
        <div class="flex items-center gap-2">
          <Skeleton class="h-3 w-20" />
          <Skeleton class="h-3 w-16" />
        </div>
      </div>
    </div>
  {/each}
</div>
