<script lang="ts">
import { Skeleton } from "@recast/ui/skeleton";
import { cn } from "@recast/ui/utils";
import { GRID_CLASS } from "$lib/library/card-styles";

interface Props {
	view: "grid" | "list";
	count?: number;
}

let { view, count = 8 }: Props = $props();
</script>

<div class={cn("grid gap-3", view === "grid" ? GRID_CLASS : "grid-cols-1")}>
  {#each { length: count } as _, i (i)}
    <Skeleton
      class={cn(view === "grid" ? "aspect-video" : "h-16")}
      style="animation-delay: {i * 80}ms"
    />
  {/each}
</div>
