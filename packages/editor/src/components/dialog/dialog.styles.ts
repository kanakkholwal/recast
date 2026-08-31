/**
 * The dialog surface, shared by every modal in the app.
 *
 * Kept as a class string rather than folded into `DialogShell` because the
 * large surfaces (extension details, shortcuts, what's new) need their own
 * header layouts but must still look like the same object. Three different
 * radius/ring combinations had drifted in before this existed.
 */
export const DIALOG_SURFACE =
	"overflow-hidden rounded-xl p-0! ring-1 ring-border/60 shadow-(--shadow-craft-xl)";

/** Header: flush on the surface, no divider. Copy carries the separation. */
export const DIALOG_HEADER = "space-y-0 px-5 pt-5 text-left";

/** Footer: on the same surface, no bar or divider — actions right, roomy. */
export const DIALOG_FOOTER = "flex items-center justify-end gap-2 px-5 pb-5 pt-2";

/**
 * Scrolling body, flush under the header (no divider). `pb-4` stands alone for
 * footer-less dialogs. Ceiling matches the most generous pre-unification cap;
 * media dialogs pass `max-h-none` and size themselves.
 */
export const DIALOG_BODY = "max-h-[min(88vh,720px)] overflow-y-auto px-5 pt-2.5 pb-4";
