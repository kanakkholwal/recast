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
Hand-stitched: Tabler ships equivalent primitives but no "monitor with play
triad" specifically. Composes `device-desktop` with a smaller `player-play`
overlay so call sites keep the same @recast/icons semantic ("a screen showing it's
ready to play / actively showing content").
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
	<rect x="2" y="4" width="20" height="13" rx="2" />
	<path d="M8 21h8" />
	<path d="M12 17v4" />
	<path d="M10 11.5l3.5 2 -3.5 2z" fill={color} stroke="none" />
</svg>
