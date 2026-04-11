import type { RecastIcon } from "$components/recast/types";

export interface PaletteCommand {
	id: string;
	title: string;
	description?: string;
	category: string;
	icon?: RecastIcon;
	keywords?: string[];
	shortcut?: string;
	action: () => void | Promise<void>;
}

class CommandPaletteStore {
	open = $state(false);
	#commands = $state<PaletteCommand[]>([]);

	get commands(): PaletteCommand[] {
		return this.#commands;
	}

	register(command: PaletteCommand) {
		const without = this.#commands.filter((c) => c.id !== command.id);
		this.#commands = [...without, command];
	}

	registerMany(commands: PaletteCommand[]) {
		const ids = new Set(commands.map((c) => c.id));
		const without = this.#commands.filter((c) => !ids.has(c.id));
		this.#commands = [...without, ...commands];
	}

	unregister(id: string) {
		this.#commands = this.#commands.filter((c) => c.id !== id);
	}

	unregisterMany(ids: string[]) {
		const set = new Set(ids);
		this.#commands = this.#commands.filter((c) => !set.has(c.id));
	}

	clearScope(category: string) {
		this.#commands = this.#commands.filter((c) => c.category !== category);
	}

	toggle() {
		this.open = !this.open;
	}

	show() {
		this.open = true;
	}

	hide() {
		this.open = false;
	}
}

export const commandPalette = new CommandPaletteStore();

/**
 * Register contextual commands that live only while a route is mounted.
 * Call from within onMount or directly in a Svelte component script.
 */
export function useCommandPalette(commands: PaletteCommand[]) {
	commandPalette.registerMany(commands);
	return () => commandPalette.unregisterMany(commands.map((c) => c.id));
}
