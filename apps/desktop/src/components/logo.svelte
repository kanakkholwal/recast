<script lang="ts">
// The resolved mode rune, not the stored 'system', so colours track the active OS theme and update on toggle.
import { mode } from "@recast/ui/theme";
import type { SVGAttributes } from "svelte/elements";

let {
	color: colorProp,
	fill: fillProp,
	size = "512",
	...rest
}: SVGAttributes<SVGSVGElement> & {
	/** Bars colour. Defaults to the theme-appropriate contrast colour. */
	color?: string;
	/** Disc background. Defaults to the theme-appropriate contrast colour. */
	fill?: string;
	size?: string | number;
} = $props();

// `mode.current` is undefined until mode-watcher hydrates; treat that as light.
const isDark = $derived(mode.current === "dark");

// Light mode is a black disc with white bars and dark mode the inverse; explicit props still win.
const fill = $derived(fillProp ?? (isDark ? "white" : "black"));
const color = $derived(colorProp ?? (isDark ? "black" : "white"));
</script>

<svg
  viewBox="0 0 512 512"
  xmlns="http://www.w3.org/2000/svg"
  {...rest}
  {fill}
  width={size}
  height={size}
>
  <rect width="512" height="512" rx="256" {fill} />
  <rect x="230" y="166" width="60" height="180" rx="30" fill={color} />
  <rect x="111" y="166" width="60" height="180" rx="30" fill={color} />
  <rect x="349" y="166" width="60" height="180" rx="30" fill={color} />
</svg>
