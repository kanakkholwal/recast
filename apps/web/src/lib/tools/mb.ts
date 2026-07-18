/**
 * Re-export shim. The canonical implementation lives in `@recast/media`
 * (see packages/media/src/input.ts and packages/media/src/conversion.ts).
 * Kept under `apps/web/src/lib/tools/` so existing call sites compile
 * unchanged; the apps/web app should prefer the bare `@recast/media`
 * import for new code.
 */

export type { ContainerKind, ConversionParams, MediaSource } from '@recast/media';
export {
	inputContainerKind,
	openInput,
	outputFormatFor,
	runConversion,
	withExtension,
} from '@recast/media';
