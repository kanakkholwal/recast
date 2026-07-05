/**
 * Shared "is the user typing into an editable element?" predicate.
 *
 * Used by global keyboard shortcuts (theme toggle, "/"-to-focus search) to
 * bail out when focus is inside a text field, so a bare/modified key press
 * can't hijack typing.
 */
export function isEditableTarget(target: EventTarget | null): boolean {
	if (!(target instanceof HTMLElement)) return false;
	if (target.isContentEditable) return true;
	const tag = target.tagName;
	return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT";
}
