/**
 * Reactive asset registry singleton.
 *
 * Holds built-in + extension entries keyed by {@link AssetKind}. Built-ins are
 * registered once via `lib/registry/builtins.ts`; extension contributions are
 * registered after hydration and removed on uninstall via
 * {@link Registry.unregisterExtension}. Backed by `$state` so pickers that read
 * `registry.list(kind)` re-render when an extension registers/unregisters at
 * runtime.
 */

import type { AssetKind, RegistryEntry } from "./types";

const EMPTY = (): Record<AssetKind, RegistryEntry[]> => ({
	cursor: [],
	background: [],
	gradient: [],
	color: [],
	easing: [],
	smoothing: [],
	captionPreset: [],
});

class Registry {
	#entries = $state<Record<AssetKind, RegistryEntry[]>>(EMPTY());

	/** Register one entry. Re-registering the same (kind,id) replaces it. */
	register<K extends AssetKind>(entry: RegistryEntry<K>): void {
		this.registerMany([entry as RegistryEntry]);
	}

	/** Register many entries (typically a whole catalog or one pack's
	 *  contributions). Grouped by kind; replaces any colliding ids in place. */
	registerMany(entries: RegistryEntry[]): void {
		for (const entry of entries) {
			const list = this.#entries[entry.kind];
			const idx = list.findIndex((e) => e.id === entry.id);
			if (idx >= 0) {
				// Replace in place to keep ordering stable so pickers don't jump.
				this.#entries[entry.kind] = [...list.slice(0, idx), entry, ...list.slice(idx + 1)];
			} else {
				this.#entries[entry.kind] = [...list, entry];
			}
		}
	}

	/** Drop every entry contributed by a given extension (on uninstall). */
	unregisterExtension(extId: string): void {
		for (const kind of Object.keys(this.#entries) as AssetKind[]) {
			const next = this.#entries[kind].filter(
				(e) => !(e.source.kind === "extension" && e.source.extId === extId),
			);
			if (next.length !== this.#entries[kind].length) {
				this.#entries[kind] = next;
			}
		}
	}

	/** All entries of a kind, with built-ins ALWAYS first and extension
	 *  contributions appended after (each group stable in registration order).
	 *  Guarantees the defaults show first and enabling a pack appends its assets
	 *  to the end, regardless of hydration/registration timing. */
	list<K extends AssetKind>(kind: K): RegistryEntry<K>[] {
		const all = this.#entries[kind] as RegistryEntry<K>[];
		// Common case with no extensions for this kind: return as-is rather than allocate and churn picker reactivity.
		if (!all.some((e) => e.source.kind === "extension")) return all;
		const builtins = all.filter((e) => e.source.kind !== "extension");
		const extensions = all.filter((e) => e.source.kind === "extension");
		return [...builtins, ...extensions];
	}

	/** Look up a single entry by storage id within a kind. */
	get<K extends AssetKind>(kind: K, id: string): RegistryEntry<K> | undefined {
		return this.#entries[kind].find((e) => e.id === id) as RegistryEntry<K> | undefined;
	}
}

export const registry = new Registry();
