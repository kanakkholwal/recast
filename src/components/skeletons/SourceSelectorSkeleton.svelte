<script lang="ts">
  import { Skeleton } from "$components/ui/skeleton";
</script>

<div class="grid grid-cols-2 gap-2 mt-4">
  {#each Array(6) as _, i}
    <div
      class="group relative overflow-hidden rounded-lg border border-card/6 bg-card/3"
    >
      <!-- Thumbnail -->
      <div class="relative aspect-video w-full overflow-hidden bg-card/30">
        <Skeleton class="h-full w-full rounded-none bg-muted" />
      </div>

      <!-- Label -->
      <div class="px-2.5 py-2">
        <Skeleton class="h-3.5 w-3/4 bg-muted rounded-sm" />
        <Skeleton class="mt-1 h-3 w-1/3 bg-muted rounded-sm" />
      </div>
    </div>
  {/each}
</div>
