<script lang="ts">
import { cn } from "@recast/ui/utils";
import type { Snippet } from "svelte";

let {
	children,
	fill = "text-card",
	// Flip the wings upward so the shelf rises from a bottom edge instead of hanging.
	inverted = false,
	// Dock vertically to a side edge: wings on top/bottom, content stacked.
	vertical = false,
	class: className = "",
}: {
	children?: Snippet;
	/** currentColor for the bar + wings; must match the panel it bridges into. */
	fill?: string;
	inverted?: boolean;
	vertical?: boolean;
	class?: string;
} = $props();

const WING =
	"M50 45C57.3095 56.6952 71.2084 63.9997 85 64V0H0C13.7915 0 26.6905 7.30481 34 19L50 45Z";
</script>

{#if vertical}
  <!-- Vertical dock: a rounded card, content stacked upright. The winged bridge
       is horizontal-edge geometry, so the side dock reads as a clean raised card. -->
  <div
    class={cn(
      "relative z-10 my-auto w-fit rounded-xl shadow-craft-md ring-1 ring-inset ring-border/40",
      fill,
      className,
    )}
  >
    <div class="flex flex-col items-center justify-center rounded-xl bg-current p-0.5">
      <div class="flex flex-col items-center justify-center text-foreground">
        {@render children?.()}
      </div>
    </div>
  </div>
{:else}
  <div
    class={cn(
      "relative z-10 mx-auto flex h-11 w-fit -translate-y-px items-start justify-center",
      inverted && "-scale-y-100",
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
      <rect x="0" y="0" width="85" height="1" fill="currentColor" transform="translate(0, -1)" />
      <path d={WING} fill="currentColor" />
    </svg>

    <!-- `bg-current` inherits the shelf's own fill colour; the children's colour
         reset lives on the nested element so it can't repaint the bar. -->
    <div
      class="relative z-10 flex h-[calc(100%+1px)] min-w-0 grow items-center justify-center bg-current"
    >
      <div class="flex items-center justify-center text-foreground">
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
      <rect x="0" y="0" width="85" height="1" fill="currentColor" transform="translate(0, -1)" />
      <path d={WING} fill="currentColor" />
    </svg>
  </div>
{/if}
