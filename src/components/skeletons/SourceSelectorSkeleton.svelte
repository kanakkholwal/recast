<script lang="ts">
  import { Skeleton } from "$components/ui/skeleton";
</script>

<div class="p-4 space-y-4">
  <!-- Tab bar -->
  <div class="flex gap-2">
    <Skeleton class="h-8 w-24 rounded-md" />
    <Skeleton class="h-8 w-24 rounded-md" />
  </div>

  <!-- Source cards grid -->
  <div class="grid grid-cols-2 gap-3">
    {#each Array(4) as _, i}
      <div
        class="rounded-xl border border-border bg-card overflow-hidden"
        style="animation-delay: {i * 50}ms"
      >
        <Skeleton class="aspect-video w-full rounded-none" />
        <div class="p-3 space-y-2">
          <Skeleton class="h-4 w-3/4" />
          <Skeleton class="h-3 w-1/2" />
        </div>
      </div>
    {/each}
  </div>
</div>
