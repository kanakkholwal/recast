<script lang="ts">
import { cn } from "../utils";
import type { IconProps } from "../types";

let {
	class: klass = "",
	size = 24,
	stroke = 2,
	color = "currentColor",
	"aria-hidden": ariaHidden = "true",
	...rest
}: IconProps = $props();
</script>

<!--
Hand-stitched: Lucide had a 3-axis move icon (X/Y/Z arrows hinting at 3D
movement). Tabler's `arrows-move` is 2D only; this is a composite that keeps
the 3D affordance for the cursor / scene-anim 3D handles we use it for.
-->
<svg
	xmlns="http://www.w3.org/2000/svg"
	width={size}
	height={size}
	viewBox="0 0 24 24"
	fill="none"
	stroke={color}
	stroke-width={stroke}
	stroke-linecap="round"
	stroke-linejoin="round"
	aria-hidden={ariaHidden}
	class={cn("shrink-0", klass)}
	{...rest}
>
	<!-- X axis (left-right) -->
	<path d="M3 12h18" />
	<polyline points="6,9 3,12 6,15" />
	<polyline points="18,9 21,12 18,15" />
	<!-- Y axis (up-down on the right) -->
	<path d="M15 5l-3 3 -3 -3" />
	<path d="M15 19l-3 -3 -3 3" />
	<!-- Z axis (depth, dashed front-back) -->
	<path d="M5 6l4 -2" />
	<path d="M5 18l4 2" />
</svg>
