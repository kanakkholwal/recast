<script lang="ts">
import type { IconComponent } from "@recast/icons";
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
Hand-stitched icon: Tabler as unknown as IconComponent as unknown as IconComponent has "grid-3x3", "grid-pattern", and "grid-dots" but
no 2x2 variant. Authored as a four-cell square placeholder; visually consistent
with @recast/ui's other stroke-based 24x24 icons.
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
	<rect x="3" y="3" width="18" height="18" rx="2" />
	<line x1="12" y1="3" x2="12" y2="21" />
	<line x1="3" y1="12" x2="21" y2="12" />
</svg>
