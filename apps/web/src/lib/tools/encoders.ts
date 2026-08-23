/**
 * Re-export shim. The canonical implementation lives in `@recast/media`
 * (see packages/media/src/encoders.ts). Kept under
 * `apps/web/src/lib/tools/` so existing call sites compile unchanged;
 * the apps/web app should prefer the bare `@recast/media` import for
 * new code.
 */

export type { GifWriter } from "@recast/media";
export {
	createGifWriter,
	encodeMp3,
	encodeWav,
	zipFiles,
} from "@recast/media";
