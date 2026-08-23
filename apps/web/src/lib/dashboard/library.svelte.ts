import type { FolderDTO, TagDTO } from "./api";

/**
 * Library organization stores — the workspace's folder tree and tag set.
 * Siblings of `recastsStore`. Hydrated from the page loader; mutated
 * optimistically by components after the matching `api.ts` call resolves.
 *
 * Folders use the same ID-based materialized `path` as the server
 * ("/<id>/<id>/"), so subtree math (descendants, delete) is a prefix test.
 */

export type Folder = FolderDTO;
export type Tag = TagDTO;

class FoldersStore {
	items = $state<Folder[]>([]);
	hydrated = $state(false);

	hydrate(server: Folder[]) {
		this.items = server;
		this.hydrated = true;
	}

	get(id: string): Folder | undefined {
		return this.items.find((f) => f.id === id);
	}

	childrenOf(parentId: string | null): Folder[] {
		return this.items
			.filter((f) => f.parentId === parentId)
			.sort((a, b) => a.name.localeCompare(b.name));
	}

	/** A folder + every descendant (by path prefix). */
	subtreeIds(id: string): Set<string> {
		const root = this.get(id);
		if (!root) return new Set([id]);
		return new Set(this.items.filter((f) => f.path.startsWith(root.path)).map((f) => f.id));
	}

	/** Breadcrumb names from root → folder, for the header trail. */
	breadcrumb(id: string | null): Folder[] {
		const trail: Folder[] = [];
		let cur = id ? this.get(id) : undefined;
		while (cur) {
			trail.unshift(cur);
			cur = cur.parentId ? this.get(cur.parentId) : undefined;
		}
		return trail;
	}

	add(f: Folder) {
		this.items = [...this.items, f];
	}

	update(id: string, patch: Partial<Folder>) {
		this.items = this.items.map((f) => (f.id === id ? { ...f, ...patch } : f));
	}

	/** Remove a folder and its whole subtree (mirrors the server cascade). */
	remove(id: string): Set<string> {
		const ids = this.subtreeIds(id);
		this.items = this.items.filter((f) => !ids.has(f.id));
		return ids;
	}
}

class TagsStore {
	items = $state<Tag[]>([]);
	hydrated = $state(false);

	hydrate(server: Tag[]) {
		this.items = server;
		this.hydrated = true;
	}

	get(id: string): Tag | undefined {
		return this.items.find((t) => t.id === id);
	}

	get sorted(): Tag[] {
		return [...this.items].sort((a, b) => a.name.localeCompare(b.name));
	}

	add(t: Tag) {
		// Idempotent on id — the create endpoint returns existing on conflict.
		if (this.items.some((x) => x.id === t.id)) return;
		this.items = [...this.items, t];
	}

	update(id: string, patch: Partial<Tag>) {
		this.items = this.items.map((t) => (t.id === id ? { ...t, ...patch } : t));
	}

	remove(id: string) {
		this.items = this.items.filter((t) => t.id !== id);
	}
}

export const foldersStore = new FoldersStore();
export const tagsStore = new TagsStore();
