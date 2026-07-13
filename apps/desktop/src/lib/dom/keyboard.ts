// Rules about who owns a keystroke.
//
// Window-level handlers are greedy: they see every key, including ones a focused
// control or a layer above them has a stronger claim on. These predicates decide
// when a global handler must stand down. The pure rules are separated from the
// DOM adapters so they stay unit-testable in the Node test environment.

/**
 * Overlay layers, in z-order above the page. Every one of these is unmounted
 * when closed, so finding one in the DOM means it is open.
 *
 * `role="dialog"` / `role="alertdialog"` catch hand-rolled modals (PresetPicker)
 * that don't come from the `@recast/ui` package.
 */
export const OVERLAY_SELECTOR = [
	'[data-slot="dialog-content"]',
	'[data-slot="alert-dialog-content"]',
	'[data-slot="sheet-content"]',
	'[data-slot="popover-content"]',
	'[data-slot="dropdown-menu-content"]',
	'[data-slot="context-menu-content"]',
	'[data-slot="select-content"]',
	'[role="dialog"]',
	'[role="alertdialog"]',
].join(",");

/**
 * True when any overlay layer is open. The topmost layer owns Escape, so a
 * window-level Escape handler underneath it must not also fire.
 *
 * Without this, dismissing a dialog opened over the export flow falls through to
 * the export panel's own Escape handler, which cancels the running export.
 */
export function isOverlayOpen(root: ParentNode = document): boolean {
	return root.querySelector(OVERLAY_SELECTOR) !== null;
}

/**
 * Native elements the browser itself activates with Space. Buttons fire their
 * click on Space *keyup*, so a global `preventDefault()` on Space keydown
 * silently makes every button in the app unusable by keyboard.
 *
 * Deliberately native tags only. Elements carrying `role="button"` but no key
 * handler of their own (the timeline's clip blocks) do nothing with Space, so
 * yielding to them would just swallow the key.
 */
export function tagActivatesOnSpace(tagName: string, hasHref = false): boolean {
	switch (tagName.toUpperCase()) {
		case "BUTTON":
		case "INPUT":
		case "SELECT":
		case "TEXTAREA":
		case "SUMMARY":
			return true;
		case "A":
			return hasHref;
		default:
			return false;
	}
}

/** DOM adapter for `tagActivatesOnSpace`; also yields to contenteditable hosts. */
export function activatesOnSpace(el: Element | null): boolean {
	if (!el) return false;
	if ((el as HTMLElement).isContentEditable) return true;
	return tagActivatesOnSpace(el.tagName, el.hasAttribute("href"));
}
