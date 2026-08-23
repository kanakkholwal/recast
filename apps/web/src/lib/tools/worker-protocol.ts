/**
 * Re-export shim. The canonical implementation lives in `@recast/media`
 * (see packages/media/src/protocol.ts). Kept under `apps/web/src/lib/tools/`
 * so existing call sites compile unchanged; the apps/web app should
 * prefer the bare `@recast/media` import for new code.
 */
export {
	ConvertError,
	type ConvertErrorCode,
	type ConvertHandler,
	type ConvertJob,
	type FromConvertWorker,
	type HandlerResult,
	type JobContext,
	type ToConvertWorker,
	type ToolOp,
	type ToolOptions,
} from "@recast/media";
