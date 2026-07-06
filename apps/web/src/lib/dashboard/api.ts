/**
 * Thin fetch wrappers for the dashboard's recast / folder / tag mutations.
 * Each throws an Error with the server message on non-2xx so callers can
 * `toast.error(e.message)`. Components call these, then update the local
 * stores optimistically.
 */

async function jsonOrThrow<T>(res: Response): Promise<T> {
	if (!res.ok) {
		const message = await res.text().catch(() => "");
		throw new Error(message || `Request failed (${res.status})`);
	}
	return (await res.json()) as T;
}

function post(url: string, body: unknown) {
	return fetch(url, {
		method: "POST",
		headers: { "content-type": "application/json" },
		body: JSON.stringify(body),
	});
}
function patch(url: string, body: unknown) {
	return fetch(url, {
		method: "PATCH",
		headers: { "content-type": "application/json" },
		body: JSON.stringify(body),
	});
}
function put(url: string, body: unknown) {
	return fetch(url, {
		method: "PUT",
		headers: { "content-type": "application/json" },
		body: JSON.stringify(body),
	});
}

// ── Recasts ──────────────────────────────────────────────────────────
export async function renameRecast(id: string, title: string): Promise<void> {
	await jsonOrThrow(await patch(`/api/recasts/${id}`, { title }));
}
export async function moveRecast(id: string, folderId: string | null): Promise<void> {
	await jsonOrThrow(await patch(`/api/recasts/${id}`, { folderId }));
}
export async function deleteRecast(id: string): Promise<void> {
	await jsonOrThrow(await fetch(`/api/recasts/${id}`, { method: "DELETE" }));
}
/** Archive a recast — deletes the video blob + reclaims storage, keeps the
 *  row (analytics/metadata) until the cron hard-deletes it. Not reversible. */
export async function archiveRecast(id: string): Promise<void> {
	await jsonOrThrow(await post(`/api/recasts/${id}/archive`, {}));
}
export async function setRecastTags(id: string, tagIds: string[]): Promise<void> {
	await jsonOrThrow(await put(`/api/recasts/${id}/tags`, { tagIds }));
}
/** Mint a share link for a recast. Default visibility matches the upload flow
 *  ("public"). Returns the slug + the absolute shareUrl the server built. */
export async function shareRecast(
	id: string,
	visibility: "private" | "workspace" | "selected" | "public" = "public",
): Promise<{ slug: string; shareUrl: string }> {
	return jsonOrThrow(await post(`/api/recasts/${id}/share`, { visibility }));
}
/** Revoke a share link by slug. */
export async function deleteShare(slug: string): Promise<void> {
	await jsonOrThrow(await fetch(`/api/share/${slug}`, { method: "DELETE" }));
}

// ── Folders ──────────────────────────────────────────────────────────
export type FolderDTO = {
	id: string;
	parentId: string | null;
	name: string;
	color: string | null;
	path: string;
};
export async function createFolder(input: {
	workspaceId: string;
	name: string;
	parentId?: string | null;
	color?: string | null;
}): Promise<FolderDTO> {
	const { folder } = await jsonOrThrow<{ folder: FolderDTO }>(
		await post(`/api/folders`, input),
	);
	return folder;
}
export async function updateFolder(
	id: string,
	patchBody: { name?: string; color?: string | null; parentId?: string | null },
): Promise<void> {
	await jsonOrThrow(await patch(`/api/folders/${id}`, patchBody));
}
export async function deleteFolder(id: string): Promise<void> {
	await jsonOrThrow(await fetch(`/api/folders/${id}`, { method: "DELETE" }));
}

// ── Tags ─────────────────────────────────────────────────────────────
export type TagDTO = { id: string; name: string; color: string | null };
export async function createTag(input: {
	workspaceId: string;
	name: string;
	color?: string | null;
}): Promise<TagDTO> {
	const { tag } = await jsonOrThrow<{ tag: TagDTO }>(await post(`/api/tags`, input));
	return tag;
}
export async function updateTag(
	id: string,
	patchBody: { name?: string; color?: string | null },
): Promise<void> {
	await jsonOrThrow(await patch(`/api/tags/${id}`, patchBody));
}
export async function deleteTag(id: string): Promise<void> {
	await jsonOrThrow(await fetch(`/api/tags/${id}`, { method: "DELETE" }));
}
