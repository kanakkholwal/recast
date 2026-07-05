/**
 * `Cmd/Ctrl + Shift + L` keychord predicate for the theme toggle shortcut.
 */
export function isThemeToggleChord(e: KeyboardEvent): boolean {
	// `key` lowercases to "l" regardless of Shift, so we don't need to
	// compare against "L". Match either Cmd (mac) or Ctrl (win/linux).
	if (e.key?.toLowerCase() !== "l") return false;
	if (!e.shiftKey) return false;
	return e.metaKey || e.ctrlKey;
}
