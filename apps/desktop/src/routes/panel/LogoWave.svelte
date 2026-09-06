<script lang="ts">
import { mode } from "@recast/ui/theme";
import { cn } from "@recast/ui/utils";
import type { SVGAttributes } from "svelte/elements";

let {
	active = false,
	size = "20",
	class: className,
	...rest
}: SVGAttributes<SVGSVGElement> & {
	/** Animate the three bars as a live mic meter, tinted success-green. */
	active?: boolean;
	size?: string | number;
} = $props();

const isDark = $derived(mode.current === "dark");
const disc = $derived(isDark ? "white" : "black");
const barContrast = $derived(isDark ? "black" : "white");
</script>

<svg
  viewBox="0 0 512 512"
  xmlns="http://www.w3.org/2000/svg"
  {...rest}
  width={size}
  height={size}
  class={cn(active && "wave text-success", className)}
>
  <rect width="512" height="512" rx="256" fill={disc} />
  <rect
    class="bar b1"
    x="111"
    y="166"
    width="60"
    height="180"
    rx="30"
    fill={active ? "currentColor" : barContrast}
  />
  <rect
    class="bar b2"
    x="230"
    y="166"
    width="60"
    height="180"
    rx="30"
    fill={active ? "currentColor" : barContrast}
  />
  <rect
    class="bar b3"
    x="349"
    y="166"
    width="60"
    height="180"
    rx="30"
    fill={active ? "currentColor" : barContrast}
  />
</svg>

<style>
  .bar {
    transform-box: fill-box;
    transform-origin: center;
  }
  .wave .bar {
    animation: wave 820ms ease-in-out infinite;
  }
  .wave .b1 {
    animation-delay: 0ms;
  }
  .wave .b2 {
    animation-delay: 150ms;
  }
  .wave .b3 {
    animation-delay: 70ms;
  }
  @keyframes wave {
    0%,
    100% {
      transform: scaleY(0.42);
    }
    50% {
      transform: scaleY(1);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .wave .bar {
      animation: none;
      transform: scaleY(0.7);
    }
  }
</style>
