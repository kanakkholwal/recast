/**
 * `recast://` deep-link dispatch.
 *
 * The OS transport (cold start via getCurrent, warm via onOpenUrl, second-
 * instance forwarding) is owned by `@tauri-apps/plugin-deep-link`; this module
 * turns a URL into an app action. The pure parser lives in `deepLink.logic.ts`
 * (framework-free, unit-tested); dispatch here touches the window/router.
 */
import { goto } from "$app/navigation";
import { parseDeepLink } from "$lib/deepLink.logic";
import { openProjectFromExternalPath } from "$lib/openProject";

export type { DeepLinkAction } from "$lib/deepLink.logic";
export {
  buildDeepLink,
  buildNavigateLink,
  buildOpenProjectLink,
  isAllowedRoute,
  parseDeepLink,
} from "$lib/deepLink.logic";

/**
 * Parse then act. Project opens spawn a fresh editor window (via
 * `openProjectFromExternalPath`); navigations move the main window's router.
 * Unrecognised links are logged and dropped.
 */
export async function handleDeepLink(raw: string): Promise<void> {
  const action = parseDeepLink(raw);
  if (!action) {
    console.warn("[deep-link] ignored", raw);
    return;
  }
  if (action.kind === "open-project") {
    await openProjectFromExternalPath(action.path);
  } else {
    await goto(action.route);
  }
}
