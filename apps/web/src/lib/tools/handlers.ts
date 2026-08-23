/**
 * Re-export shim. The canonical implementation lives in `@recast/media`
 * (see packages/media/src/handlers.ts). Kept under
 * `apps/web/src/lib/tools/` so existing call sites compile unchanged;
 * the apps/web app should prefer the bare `@recast/media` import for
 * new code.
 */
export { handlers } from "@recast/media";
