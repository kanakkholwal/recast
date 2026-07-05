/**
 * Shared command-palette open state. Mirrors the desktop app's model: one
 * store drives a single dialog (CommandPaletteHost), and any number of
 * triggers (header search, hero search) just call `show()`. Keeps the opened
 * panel identical no matter where it's launched from.
 */
class CommandPaletteStore {
	open = $state(false);

	show() {
		this.open = true;
	}

	hide() {
		this.open = false;
	}

	toggle() {
		this.open = !this.open;
	}
}

export const commandPalette = new CommandPaletteStore();
