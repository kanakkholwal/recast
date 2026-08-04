/**
 * Asset registry: public entry point.
 *
 * Importing `$lib/registry` guarantees the built-in catalogs are registered
 * (via the `./builtins` side effect) before any `registry.list(kind)` /
 * resolver call. Extension contributions register later, after hydration
 * (`lib/registry/extensions.ts`, Tier 1).
 *
 * NOTE: `editor-store.svelte.ts` must import the narrow `./resolve` directly
 * (not this index) to avoid an import cycle, since `./builtins` pulls in the
 * editor store's catalogs.
 */

import "./builtins";

export { registry } from "./registry.svelte";
export {
	resolveBackgroundWireValue,
	resolveCursorSprite,
	resolveCursorDataUrl,
	cursorSpriteSvg,
	cursorSpriteHotspot,
	resolveEasing,
	resolveSmoothing,
} from "./resolve";
export {
	EXT_PREFIX,
	extEntryId,
	isExtId,
	parseExtId,
	type AssetKind,
	type RegistryEntry,
	type Source,
	type CursorValue,
	type BackgroundValue,
	type GradientValue,
	type ColorValue,
	type EasingValue,
	type SmoothingValue,
} from "./types";
