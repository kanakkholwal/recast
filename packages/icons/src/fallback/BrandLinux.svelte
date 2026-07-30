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
Tux, stroke-drawn to sit with the Tabler set. Tabler ships per-distro marks
(Ubuntu, Debian) but nothing for Linux itself, and using a distro logo to mean
"Linux" is wrong — Ubuntu is one of many.
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
	<path
		d="M12 2.5a3 3 0 0 0 -3 3v1.9c0 .5 -.2 1 -.5 1.4a8.5 8.5 0 0 0 -1.5 4.8v3.4a3 3 0 0 0 3 3h4a3 3 0 0 0 3 -3v-3.4a8.5 8.5 0 0 0 -1.5 -4.8a2.4 2.4 0 0 1 -.5 -1.4v-1.9a3 3 0 0 0 -3 -3z"
	/>
	<path d="M10.5 6h.01" />
	<path d="M13.5 6h.01" />
	<path d="M11 8.2h2" />
	<path d="M9.5 20l-1.5 1.5" />
	<path d="M14.5 20l1.5 1.5" />
</svg>
