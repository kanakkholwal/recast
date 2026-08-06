/**
 * The dialog surface, shared by every modal in the app.
 *
 * Kept as a class string rather than folded into `DialogShell` because the
 * large surfaces (extension details, shortcuts, what's new) need their own
 * header layouts but must still look like the same object. Three different
 * radius/ring combinations had drifted in before this existed.
 */
export const DIALOG_SURFACE =
	"overflow-hidden rounded-2xl p-0! ring-1 ring-border/60 shadow-(--shadow-craft-inset-strong)";

/** Header bar: flush, bordered, left-aligned. */
export const DIALOG_HEADER = "space-y-0 border-b border-border/40 px-4 py-3.5 text-left";

/** Footer bar: bordered, muted, actions right. */
export const DIALOG_FOOTER =
	"flex items-center justify-end gap-2 border-t border-border/40 bg-muted/30 px-3 py-2.5";

/**
 * Scrolling body. The ceiling matches the most generous one the dialogs used
 * before they were unified — a tighter cap shrank the share and player dialogs.
 * Media dialogs pass `max-h-none` and size themselves.
 */
export const DIALOG_BODY = "max-h-[min(88vh,720px)] overflow-y-auto px-4 py-3.5";
