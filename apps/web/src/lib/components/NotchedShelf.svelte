<script lang="ts">
import { cn } from "@recast/ui/utils";
import type { Snippet } from "svelte";

let {
	children,
	fill = "text-background",
	class: className = "",
}: {
	children?: Snippet;
	fill?: string;
	class?: string;
} = $props();

const WING =
	"M50 45C57.3095 56.6952 71.2084 63.9997 85 64V0H0C13.7915 0 26.6905 7.30481 34 19L50 45Z";
</script>

<div
  class={cn(
    "relative z-10 mx-auto flex h-16 max-w-[min(700px,calc(100vw-2rem))] -translate-y-px items-start justify-center",
    fill,
    className,
  )}
>
  <svg
    viewBox="0 0 85 64"
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
    aria-hidden="true"
    class="h-full w-auto shrink-0 translate-x-px translate-y-px overflow-visible"
  >
    <rect
      x="0"
      y="0"
      width="85"
      height="1"
      fill="currentColor"
      transform="translate(0, -1)"
    />
    <path d={WING} fill="currentColor" />
  </svg>


  <!-- `bg-current` must inherit the shelf's own fill colour, so the content's
       colour reset has to live on a nested element: setting text-foreground
       here would repaint the bar itself near-black. -->
  <div
    class="relative z-10 flex h-[calc(100%+1px)] min-w-0 grow items-center justify-center bg-current"
  >
    <div class="flex items-center justify-center gap-2 text-foreground">
      {@render children?.()}
    </div>
  </div>
  <svg
    viewBox="0 0 85 64"
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
    aria-hidden="true"
    class="h-full w-auto shrink-0 -translate-x-px translate-y-px -scale-x-100 overflow-visible"
  >
    <rect
      x="0"
      y="0"
      width="85"
      height="1"
      fill="currentColor"
      transform="translate(0, -1)"
    />
    <path d={WING} fill="currentColor" />
  </svg>
</div>
