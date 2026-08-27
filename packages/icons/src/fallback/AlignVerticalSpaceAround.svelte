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
Hand-stitched: Lucide had `align-vertical-space-around` for stacked-with-
vertical-spacing alignment. Tabler's `space-between` and `align-center`
approximate it but with subtly different geometry. Kept as its own component
so the editor's geometry panel doesn't depend on a third-party equivalent.
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
	<rect x="6" y="3" width="12" height="4" />
	<rect x="6" y="17" width="12" height="4" />
	<line x1="12" y1="9" x2="12" y2="13" />
	<polyline points="9,11 12,14 15,11" />
</svg>
