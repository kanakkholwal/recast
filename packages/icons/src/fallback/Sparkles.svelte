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
Hand-stitched: This slot is reserved for AI-surface accents. The barrel
defers `Sparkles` to this component so the codemod can recognize the name
unambiguously even when no call site uses it as the "AI" accent; the actual
Phosphor duotone replaces it via the alias map below.

If/when Sparkles truly becomes a generic non-AI glyph (e.g. on a "what's
new" badge), point this re-export to a first-party Tabler-shaped glyph.
For now it's a four-point star so the import resolves at the type level.
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
	<path d="M12 3v4" />
	<path d="M12 17v4" />
	<path d="M3 12h4" />
	<path d="M17 12h4" />
	<path d="M5.6 5.6l2.8 2.8" />
	<path d="M15.6 15.6l2.8 2.8" />
	<path d="M5.6 18.4l2.8 -2.8" />
	<path d="M15.6 8.4l2.8 -2.8" />
</svg>
