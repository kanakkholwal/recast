import type { Component } from "svelte";

/**
 * Standard props accepted by every icon component shipped from this package.
 *
 * Loose by design so @recast/icons's call shape (`class` + `size` + `strokeWidth`)
 * maps cleanly onto Tabler's (`class` + `size` + `stroke`) without per-call
 * rewriting. `stroke` accepts `string | number` to match Tabler's signature
 * and let consumers pass either.
 */
export interface IconProps {
	class?: string;
	size?: number | string;
	stroke?: number | string;
	color?: string;
	"aria-hidden"?: boolean | "true" | "false";
	"aria-label"?: string;
	fill?: string;
}

/**
 * Anything we can drop into a slot where today a @recast/icons icon goes.
 *
 * Re-exports through this package come from Tabler, whose icons are typed
 * as Svelte 4 class components — structurally incompatible with Svelte 5's
 * `Component<Props>` interface type. To keep the call-site shape unchanged
 * from the @recast/icons era (where icons are stored in data tables as values),
 * we widen the type to `any` for the storage-side, so a @recast/icons call like
 * `icons: { icon: Home }` type-checks without per-site casts. The
 * ergonomic narrowing (so consumers can still express
 * `Component<IconProps>` slots) is preserved by the union leg.
 */
// biome-ignore lint/suspicious/noExplicitAny: Svelte 4 class vs Svelte 5 Component mismatch — see comment above.
export type IconComponent = Component<IconProps> | any;
