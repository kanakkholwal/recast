<script lang="ts" module>
import type { SVGAttributes } from "svelte/elements";

/**
 * Icon geometry as data, mirroring `@lucide/svelte`'s `IconNode`: an array of
 * `[svgTag, attributes]` tuples. Authoring an icon is pure data — no new
 * component file — which keeps the local set extensible and tree-shakeable.
 */
export type IconNode = [tag: string, attrs: Record<string, string | number>][];

export interface LocalIconProps extends SVGAttributes<SVGSVGElement> {
	size?: number | string;
	color?: string;
	strokeWidth?: number | string;
	absoluteStrokeWidth?: boolean;
	iconNode?: IconNode;
}
</script>

<script lang="ts">
  // Lucide-compatible custom-icon base. The prop surface matches a Lucide icon
  // (`size` / `color` / `strokeWidth` / `absoluteStrokeWidth` / `class` +
  // currentColor stroke), so a `<LocalIcon iconNode={…} />` drops into the same
  // call sites and a `size-*` class overrides the width/height attrs via CSS.
  // Documented exception to AGENTS.md §4 "Lucide icons only", alongside
  // `brand-icons` — for first-party glyphs Lucide doesn't ship.
  import { cn } from "@recast/ui/utils";
  import type { Snippet } from "svelte";

  let {
    size = 24,
    color = "currentColor",
    strokeWidth = 2,
    absoluteStrokeWidth = false,
    iconNode = [],
    class: className,
    children,
    ...rest
  }: LocalIconProps & { children?: Snippet } = $props();
</script>

<svg
  xmlns="http://www.w3.org/2000/svg"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke={color}
  stroke-width={absoluteStrokeWidth
    ? (Number(strokeWidth) * 24) / Number(size)
    : strokeWidth}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
  class={cn("shrink-0", className)}
  {...rest}
>
  {#each iconNode as [tag, attrs], i (i)}
    <svelte:element this={tag} {...attrs} />
  {/each}
  {@render children?.()}
</svg>
