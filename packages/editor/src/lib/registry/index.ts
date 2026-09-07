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
	cursorSpriteHotspot,
	cursorSpriteSvg,
	resolveBackgroundWireValue,
	resolveCursorDataUrl,
	resolveCursorSprite,
	resolveEasing,
	resolveSmoothing,
} from "./resolve";
export {
	type AssetKind,
	type BackgroundValue,
	type ColorValue,
	type CursorValue,
	type EasingValue,
	EXT_PREFIX,
	extEntryId,
	type GradientValue,
	isExtId,
	parseExtId,
	type RegistryEntry,
	type SmoothingValue,
	type Source,
} from "./types";
