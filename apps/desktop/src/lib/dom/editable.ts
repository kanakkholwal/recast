/**
 * True when an event target is a text-entry surface (input / textarea / select /
 * contenteditable). Global hotkeys check this so typing doesn't trigger them.
 */
export function isEditableTarget(target: EventTarget | null): boolean {
	if (!(target instanceof HTMLElement)) return false;
	if (target.isContentEditable) return true;
	const tag = target.tagName;
	return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT";
}
