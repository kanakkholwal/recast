<script lang="ts">
  import { Skeleton } from "@recast/ui/skeleton";
</script>

<div class="grid grid-cols-2 gap-3.5">
  {#each Array(6) as _, i}
    <div
      class="group relative overflow-hidden rounded-[24px] border border-border-subtle bg-foreground/[0.02]"
    >
      <!-- Thumbnail -->
      <div class="relative aspect-[16/10] w-full overflow-hidden bg-foreground/[0.05]">
        <Skeleton class="h-full w-full rounded-none opacity-20" />
      </div>

      <!-- Label -->
      <div class="px-5 py-4.5">
        <Skeleton class="h-3.5 w-3/4 opacity-20" />
        <Skeleton class="mt-1.5 h-3 w-1/3 opacity-20" />
      </div>
    </div>
  {/each}
</div>
