/**
 * `Cmd/Ctrl + Shift + L` keychord predicate for the theme toggle shortcut.
 */
export function isThemeToggleChord(e: KeyboardEvent): boolean {
	// `key` lowercases regardless of Shift, so match either Cmd or Ctrl with the plain letter.
	if (e.key?.toLowerCase() !== "l") return false;
	if (!e.shiftKey) return false;
	return e.metaKey || e.ctrlKey;
}
