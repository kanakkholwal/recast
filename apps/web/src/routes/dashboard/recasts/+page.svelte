<script lang="ts">
import {
	ChevronRight,
	Folder,
	FolderPlus,
	Inbox,
	Library,
	MoreHorizontal,
	Pencil,
	Trash2,
	Upload,
	UploadCloud,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import * as DropdownMenu from "@recast/ui/dropdown-menu";
import { toast } from "@recast/ui/sonner";
import { cn } from "@recast/ui/utils";
import { untrack } from "svelte";
import { cubicOut } from "svelte/easing";
import { fly, slide } from "svelte/transition";
import { invalidateAll } from "$app/navigation";
import * as api from "$lib/dashboard/api";
import ConfirmDialog from "$lib/dashboard/components/ConfirmDialog.svelte";
import LibraryToolbar from "$lib/dashboard/components/LibraryToolbar.svelte";
import PageHeader from "$lib/dashboard/components/PageHeader.svelte";
import RecastGrid from "$lib/dashboard/components/RecastGrid.svelte";
import RenameDialog from "$lib/dashboard/components/RenameDialog.svelte";
import SelectionBar from "$lib/dashboard/components/SelectionBar.svelte";
import TagManagerDialog from "$lib/dashboard/components/TagManagerDialog.svelte";
import { mapRecastsForStore } from "$lib/dashboard/hydrate";
import { foldersStore, tagsStore } from "$lib/dashboard/library.svelte";
import { POSTER_ACCEPT, replacePoster } from "$lib/dashboard/poster";
import { quickUpload } from "$lib/dashboard/quick-upload.svelte";
import { filterAndSortRecasts, isFileDrag } from "$lib/dashboard/recasts-library.logic";
import { type Recast, recastsStore } from "$lib/dashboard/store.svelte";

type FolderSelection = "all" | "root" | string;

let { data } = $props();

// Hydrate recasts + folders + tags from the server.
$effect(() => {
	const mapped = mapRecastsForStore(data.recasts);
	const folders = data.folders;
	const tags = data.tags;
	const ws = data.workspaceId;
	untrack(() => {
		recastsStore.hydrate(mapped, ws);
		foldersStore.hydrate(folders);
		tagsStore.hydrate(tags);
	});
});

const workspaceId = $derived(data.workspaceId);

let viewMode = $state<"grid" | "list">("grid");
let query = $state("");
let sortKey = $state<string>("recent");
let selectedFolder = $state<FolderSelection>("all");
let selectedTagIds = $state<string[]>([]);

let renaming = $state<Recast | null>(null);
// Delete and archive both destroy something no undo can bring back.
let confirmDelete = $state<Recast | null>(null);
let confirmArchive = $state<Recast | null>(null);
let confirmBulkDelete = $state(false);
let acting = $state(false);
let managingTags = $state(false);
let creatingFolder = $state(false);
let creatingParentId = $state<string | null>(null);
let renamingFolderId = $state<string | null>(null);
let newFolderName = $state("");

// Bulk selection.
let selectedIds = $state(new Set<string>());
const selectionMode = $derived(selectedIds.size > 0);

// Drag-and-drop onto the library opens the shared upload dialog with the
// dropped file staged; the header + empty-state buttons open it empty.
let dragDepth = $state(0);
const isDraggingFile = $derived(dragDepth > 0);

const searching = $derived(query.trim() !== "");

// A folder browser that hid matches in other folders would be a search box
// that lies, so searching always scopes to the whole library.
const visible = $derived(
	filterAndSortRecasts(recastsStore.items, {
		query,
		activeFilter: "all",
		folder: searching ? "all" : selectedFolder,
		tagIds: selectedTagIds,
		sortKey,
	}),
);

const hasRecasts = $derived(recastsStore.items.length > 0);
const filtersActive = $derived(searching || selectedFolder !== "all" || selectedTagIds.length > 0);

// Only ever a real folder id, so it can parent a new subfolder.
const currentFolderId = $derived(
	selectedFolder === "all" || selectedFolder === "root" ? null : selectedFolder,
);
const folderCrumb = $derived(currentFolderId ? foldersStore.breadcrumb(currentFolderId) : []);
// One level at a time. The flat list showed a subfolder beside its own parent.
const childFolders = $derived(foldersStore.childrenOf(currentFolderId));
const unfiledCount = $derived(recastsStore.items.filter((r) => !r.folderId).length);
const showUnfiled = $derived(selectedFolder === "all" && unfiledCount > 0);

function countForFolder(folderId: string): number {
	const ids = foldersStore.subtreeIds(folderId);
	return recastsStore.items.filter((r) => r.folderId && ids.has(r.folderId)).length;
}

function clearFilters() {
	query = "";
	selectedFolder = "all";
	selectedTagIds = [];
}

// --- Selection ---
function toggleSelect(rec: Recast) {
	const next = new Set(selectedIds);
	if (next.has(rec.id)) next.delete(rec.id);
	else next.add(rec.id);
	selectedIds = next;
}
function clearSelection() {
	selectedIds = new Set();
}

// --- Upload (drag-and-drop; button/input flows go through `upload`) ---
function onDragEnter(e: DragEvent) {
	if (!isFileDrag(e)) return;
	e.preventDefault();
	dragDepth++;
}
function onDragOver(e: DragEvent) {
	if (!isFileDrag(e)) return;
	e.preventDefault();
	if (e.dataTransfer) e.dataTransfer.dropEffect = "copy";
}
function onDragLeave(e: DragEvent) {
	if (!isFileDrag(e)) return;
	dragDepth = Math.max(0, dragDepth - 1);
}
function onDrop(e: DragEvent) {
	if (!isFileDrag(e)) return;
	e.preventDefault();
	dragDepth = 0;
	const file = e.dataTransfer?.files?.[0];
	if (file) quickUpload.show(file);
}

// --- Mutations ---
async function doRename(rec: Recast, title: string) {
	renaming = null;
	const prev = rec.title;
	recastsStore.rename(rec.id, title);
	try {
		await api.renameRecast(rec.id, title);
		toast.success("Recast renamed.");
	} catch (e) {
		recastsStore.rename(rec.id, prev);
		toast.error((e as Error)?.message ?? "Couldn't rename.");
	}
}

async function moveRecast(rec: Recast, folderId: string | null) {
	if (rec.folderId === folderId) return;
	const prev = rec.folderId;
	recastsStore.move(rec.id, folderId);
	try {
		await api.moveRecast(rec.id, folderId);
		const name = folderId ? (foldersStore.get(folderId)?.name ?? "folder") : "No folder";
		toast.success(`Moved to ${name}.`);
	} catch (e) {
		recastsStore.move(rec.id, prev);
		toast.error((e as Error)?.message ?? "Couldn't move recast.");
	}
}

async function toggleTag(rec: Recast, tagId: string) {
	const prev = rec.tags;
	const next = prev.includes(tagId) ? prev.filter((t) => t !== tagId) : [...prev, tagId];
	recastsStore.setTags(rec.id, next);
	try {
		await api.setRecastTags(rec.id, next);
	} catch (e) {
		recastsStore.setTags(rec.id, prev);
		toast.error((e as Error)?.message ?? "Couldn't update tags.");
	}
}

// --- Replace poster (cloud recasts only) ---
let posterInput = $state<HTMLInputElement | null>(null);
let posterTargetId = $state<string | null>(null);

function changePoster(rec: Recast) {
	posterTargetId = rec.id;
	posterInput?.click();
}

async function onPosterPicked(e: Event) {
	const input = e.currentTarget as HTMLInputElement;
	const file = input.files?.[0];
	const id = posterTargetId;
	input.value = "";
	posterTargetId = null;
	if (!file || !id) return;
	const pending = toast.loading("Updating poster…");
	try {
		const posterUrl = await replacePoster(id, file);
		if (posterUrl) recastsStore.setPoster(id, posterUrl);
		toast.success("Poster updated.", { id: pending });
	} catch (err) {
		toast.error((err as Error)?.message ?? "Couldn't update the poster.", { id: pending });
	}
}

async function copyLink(rec: Recast) {
	try {
		let slug = rec.latestShareSlug ?? null;
		if (!slug) {
			const { slug: newSlug } = await api.shareRecast(rec.id);
			slug = newSlug;
			recastsStore.setShareSlug(rec.id, slug);
		}
		await navigator.clipboard.writeText(`${location.origin}/share/${slug}`);
		toast.success("Share link copied to clipboard.");
	} catch (e) {
		toast.error((e as Error)?.message ?? "Couldn't copy the share link.");
	}
}

async function deleteRecast(rec: Recast) {
	if (acting) return;
	acting = true;
	const snapshot = recastsStore.items;
	recastsStore.remove(rec.id);
	try {
		await api.deleteRecast(rec.id);
		confirmDelete = null;
		toast.success(`“${rec.title}” deleted.`);
		// Storage and the active-recast count changed with it.
		void invalidateAll();
	} catch (e) {
		recastsStore.hydrate(snapshot);
		toast.error((e as Error)?.message ?? "Couldn't delete recast.");
	} finally {
		acting = false;
	}
}

async function archiveRecast(rec: Recast) {
	if (acting) return;
	acting = true;
	const snapshot = recastsStore.items;
	recastsStore.remove(rec.id);
	try {
		await api.archiveRecast(rec.id);
		confirmArchive = null;
		toast.success(`“${rec.title}” archived. Storage freed.`);
		// Refresh the archive route's list + quota usage.
		void invalidateAll();
	} catch (e) {
		recastsStore.hydrate(snapshot);
		toast.error((e as Error)?.message ?? "Couldn't archive recast.");
	} finally {
		acting = false;
	}
}

// --- Bulk mutations ---
function plural(n: number) {
	return n === 1 ? "" : "s";
}

async function bulkMove(folderId: string | null) {
	const ids = [...selectedIds];
	const snapshot = recastsStore.items;
	for (const id of ids) recastsStore.move(id, folderId);
	clearSelection();
	try {
		await Promise.all(ids.map((id) => api.moveRecast(id, folderId)));
		const name = folderId ? (foldersStore.get(folderId)?.name ?? "folder") : "No folder";
		toast.success(`Moved ${ids.length} recast${plural(ids.length)} to ${name}.`);
	} catch (e) {
		recastsStore.hydrate(snapshot);
		toast.error((e as Error)?.message ?? "Couldn't move recasts.");
	}
}

async function bulkAddTag(tagId: string) {
	const ids = [...selectedIds];
	const snapshot = recastsStore.items;
	clearSelection();
	const updates = ids.map((id) => {
		const rec = snapshot.find((r) => r.id === id);
		const next = rec && !rec.tags.includes(tagId) ? [...rec.tags, tagId] : (rec?.tags ?? []);
		recastsStore.setTags(id, next);
		return { id, next };
	});
	try {
		await Promise.all(updates.map((u) => api.setRecastTags(u.id, u.next)));
		toast.success(`Tagged ${ids.length} recast${plural(ids.length)}.`);
	} catch (e) {
		recastsStore.hydrate(snapshot);
		toast.error((e as Error)?.message ?? "Couldn't tag recasts.");
	}
}

async function bulkDelete() {
	if (acting) return;
	acting = true;
	const ids = [...selectedIds];
	for (const id of ids) recastsStore.remove(id);
	clearSelection();
	// Settled, not all: one failure among ten does not un-delete the nine that
	// succeeded, so the server is the only honest source for what is left.
	const results = await Promise.allSettled(ids.map((id) => api.deleteRecast(id)));
	const failed = results.filter((r) => r.status === "rejected").length;
	acting = false;
	confirmBulkDelete = false;
	await invalidateAll();
	if (failed === 0) {
		toast.success(`Deleted ${ids.length} recast${plural(ids.length)}.`);
	} else {
		toast.error(`${failed} of ${ids.length} couldn't be deleted.`);
	}
}

async function createTag(name: string) {
	try {
		const tag = await api.createTag({ workspaceId, name });
		tagsStore.add(tag);
	} catch (e) {
		toast.error((e as Error)?.message ?? "Couldn't create tag.");
	}
}

function startCreateFolder(parentId: string | null = null) {
	creatingParentId = parentId;
	creatingFolder = true;
	renamingFolderId = null;
	newFolderName = "";
}

function cancelFolderDraft() {
	creatingFolder = false;
	creatingParentId = null;
	renamingFolderId = null;
	newFolderName = "";
}

async function createFolder() {
	const name = newFolderName.trim();
	const parentId = creatingParentId;
	cancelFolderDraft();
	if (!name) return;
	try {
		const folder = await api.createFolder({ workspaceId, name, parentId });
		foldersStore.add(folder);
		toast.success(`Folder "${name}" created.`);
	} catch (e) {
		toast.error((e as Error)?.message ?? "Couldn't create folder.");
	}
}

function startRenameFolder(folderId: string, name: string) {
	renamingFolderId = folderId;
	creatingFolder = false;
	creatingParentId = null;
	newFolderName = name;
}

async function renameFolder(folderId: string) {
	const folder = foldersStore.get(folderId);
	const name = newFolderName.trim();
	cancelFolderDraft();
	if (!folder || !name || name === folder.name) return;
	const prev = folder.name;
	foldersStore.update(folderId, { name });
	try {
		await api.updateFolder(folderId, { name });
		toast.success("Folder renamed.");
	} catch (e) {
		foldersStore.update(folderId, { name: prev });
		toast.error((e as Error)?.message ?? "Couldn't rename folder.");
	}
}

async function removeFolder(folderId: string) {
	const folder = foldersStore.get(folderId);
	if (!folder) return;
	const ids = foldersStore.subtreeIds(folderId);
	foldersStore.remove(folderId);
	recastsStore.clearFolder(ids);
	if (selectedFolder !== "all" && ids.has(selectedFolder as string)) selectedFolder = "all";
	try {
		await api.deleteFolder(folderId);
		toast.success(`Folder "${folder.name}" deleted.`);
	} catch (e) {
		toast.error((e as Error)?.message ?? "Couldn't delete folder.");
	}
}

async function commitFolderDraft(folderId: string | undefined = undefined) {
	if (folderId) {
		await renameFolder(folderId);
		return;
	}
	await createFolder();
}
</script>

<svelte:head>
	<title>Recasts - Recast Dashboard</title>
</svelte:head>

<input
	bind:this={posterInput}
	type="file"
	accept={POSTER_ACCEPT}
	class="hidden"
	onchange={onPosterPicked}
/>

<PageHeader
	icon={Library}
	title="Recasts"
	subtitle="Everything you've captured, uploaded, and shared."
>
	<Button variant="dark" class="gap-2" onclick={() => quickUpload.show()}>
		<Upload class="size-4" />
		Upload recast
	</Button>
</PageHeader>

<!-- The whole library region is a file drop target. -->
<div
	role="region"
	aria-label="Recast library"
	class="relative mt-6"
	in:fly={{ y: 12, duration: 480, delay: 80, easing: cubicOut }}
	ondragenter={onDragEnter}
	ondragover={onDragOver}
	ondragleave={onDragLeave}
	ondrop={onDrop}
>
	<!-- Where you are, and what you can add here. -->
	<div class="flex flex-wrap items-center justify-between gap-3 border-b border-border-low pb-3">
		<nav class="flex min-w-0 items-center gap-1 text-body-sm" aria-label="Folder path">
			<button
				type="button"
				onclick={() => (selectedFolder = "all")}
				class={cn(
					"inline-flex items-center gap-1.5 rounded-md px-1.5 py-1 transition-colors hover:text-foreground motion-reduce:transition-none",
					selectedFolder === "all" ? "font-medium text-foreground" : "text-muted-foreground",
				)}
			>
				<Library class="size-3.5" />
				Library
			</button>
			{#if selectedFolder === "root"}
				<ChevronRight class="size-3.5 shrink-0 text-border-strong" />
				<span class="px-1.5 py-1 font-medium text-foreground">Unfiled</span>
			{/if}
			{#each folderCrumb as f, i (f.id)}
				<ChevronRight class="size-3.5 shrink-0 text-border-strong" />
				<button
					type="button"
					onclick={() => (selectedFolder = f.id)}
					class={cn(
						"min-w-0 truncate rounded-md px-1.5 py-1 transition-colors hover:text-foreground motion-reduce:transition-none",
						i === folderCrumb.length - 1 ? "font-medium text-foreground" : "text-muted-foreground",
					)}
				>
					{f.name}
				</button>
			{/each}
		</nav>

		<Button
			variant="outline"
			size="sm"
			class="gap-2"
			onclick={() => startCreateFolder(currentFolderId)}
		>
			<FolderPlus class="size-3.5" />
			New folder
		</Button>
	</div>

	{#if childFolders.length > 0 || showUnfiled || creatingFolder}
		<div class="mt-4 grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
			{#if showUnfiled}
				<button
					type="button"
					onclick={() => (selectedFolder = "root")}
					class="surface flex min-h-15 items-center gap-3 px-3 text-left transition-colors hover:border-border-strong motion-reduce:transition-none"
				>
					<Inbox class="size-4 shrink-0 text-muted-foreground" />
					<span class="min-w-0">
						<span class="block truncate text-body-sm font-medium text-foreground">Unfiled</span>
						<span class="text-caption tabular-nums text-muted-foreground">
							{unfiledCount} video{unfiledCount === 1 ? "" : "s"}
						</span>
					</span>
				</button>
			{/if}

			{#each childFolders as folder (folder.id)}
				{@const isRenaming = renamingFolderId === folder.id}
				{@const count = countForFolder(folder.id)}
				<div
					role="listitem"
					class="surface group/folder flex min-h-15 items-center gap-3 px-3 text-left transition-colors hover:border-border-strong motion-reduce:transition-none"
					ondragover={(e) => e.preventDefault()}
					ondrop={(e) => {
						const id = e.dataTransfer?.getData("text/recast-id");
						const rec = recastsStore.items.find((r) => r.id === id);
						if (rec) moveRecast(rec, folder.id);
					}}
				>
					<button
						type="button"
						onclick={() => (selectedFolder = folder.id)}
						class="flex min-w-0 flex-1 items-center gap-3 py-3 text-left"
					>
						{#if folder.color}
							<span class="size-4 shrink-0 rounded-[4px]" style="background:{folder.color}"></span>
						{:else}
							<Folder class="size-4 shrink-0 text-muted-foreground" />
						{/if}
						<span class="min-w-0">
							{#if isRenaming}
								<input
									bind:value={newFolderName}
									onclick={(e) => e.stopPropagation()}
									onblur={() => commitFolderDraft(folder.id)}
									onkeydown={(e) => {
										if (e.key === "Enter") e.currentTarget.blur();
										if (e.key === "Escape") cancelFolderDraft();
									}}
									class="block w-full bg-transparent text-body-sm font-medium text-foreground outline-none"
								/>
							{:else}
								<span class="block truncate text-body-sm font-medium text-foreground">
									{folder.name}
								</span>
							{/if}
							<span class="text-caption tabular-nums text-muted-foreground">
								{count} video{count === 1 ? "" : "s"}
							</span>
						</span>
					</button>
					<DropdownMenu.Root>
						<DropdownMenu.Trigger
							class="grid size-8 shrink-0 place-items-center rounded-md text-muted-foreground transition-colors hover:bg-paper hover:text-foreground sm:opacity-0 sm:group-hover/folder:opacity-100 sm:focus-visible:opacity-100"
							aria-label="Folder options"
						>
							<MoreHorizontal class="size-4" />
						</DropdownMenu.Trigger>
						<DropdownMenu.Content align="end" sideOffset={6} class="w-44">
							<DropdownMenu.Item onclick={() => startRenameFolder(folder.id, folder.name)}>
								<Pencil class="size-4 text-muted-foreground" />
								Rename
							</DropdownMenu.Item>
							<DropdownMenu.Item onclick={() => startCreateFolder(folder.id)}>
								<FolderPlus class="size-4 text-muted-foreground" />
								New subfolder
							</DropdownMenu.Item>
							<DropdownMenu.Separator />
							<DropdownMenu.Item
								onclick={() => removeFolder(folder.id)}
								class="text-destructive/90 data-highlighted:text-destructive"
							>
								<Trash2 class="size-4" />
								Delete
							</DropdownMenu.Item>
						</DropdownMenu.Content>
					</DropdownMenu.Root>
				</div>
			{/each}

			{#if creatingFolder}
				<form
					class="surface flex min-h-15 items-center gap-3 border-border-strong px-3"
					onsubmit={(e) => {
						e.preventDefault();
						createFolder();
					}}
				>
					<Folder class="size-4 shrink-0 text-muted-foreground" />
					<input
						bind:value={newFolderName}
						placeholder="Folder name"
						onkeydown={(e) => {
							if (e.key === "Escape") cancelFolderDraft();
						}}
						onblur={() => commitFolderDraft()}
						class="min-w-0 flex-1 bg-transparent text-body-sm font-medium text-foreground outline-none placeholder:text-muted-foreground"
					/>
				</form>
			{/if}
		</div>
	{/if}

	<div class="mt-5">
		<LibraryToolbar
			bind:query
			bind:sortKey
			bind:selectedTagIds
			bind:viewMode
			total={recastsStore.items.length}
			shown={visible.length}
			{filtersActive}
			onclear={clearFilters}
			onmanagetags={() => (managingTags = true)}
			oncreatetag={createTag}
		/>
	</div>

	{#if searching && selectedFolder !== "all"}
		<p
			class="mt-3 text-body-sm text-muted-foreground"
			transition:slide={{ duration: 200, easing: cubicOut }}
		>
			Searching the whole library, not just this folder.
		</p>
	{/if}

	<div class="mt-5">
		<RecastGrid
			recasts={visible}
			folders={foldersStore.items}
			tags={tagsStore.items}
			{selectedIds}
			{selectionMode}
			{viewMode}
			hasAnyRecasts={hasRecasts}
			{filtersActive}
			onrename={(rec) => (renaming = rec)}
			oncopylink={copyLink}
			onchangeposter={changePoster}
			onmove={moveRecast}
			ontoggletag={toggleTag}
			onarchive={(rec) => (confirmArchive = rec)}
			ondelete={(rec) => (confirmDelete = rec)}
			onToggleSelect={toggleSelect}
			onupload={() => quickUpload.show()}
			onclearfilters={clearFilters}
		/>
	</div>

	<!-- Drop-to-upload overlay -->
	{#if isDraggingFile}
		<div
			class="pointer-events-none absolute inset-0 z-30 grid place-items-center rounded-xl border-2 border-dashed border-primary bg-background/85"
			transition:fly={{ y: 8, duration: 160, easing: cubicOut }}
		>
			<div class="flex flex-col items-center gap-2 text-center">
				<UploadCloud class="size-6 text-primary" />
				<p class="font-display text-body font-medium text-foreground">Drop to upload</p>
				<p class="text-body-sm text-muted-foreground">
					We'll upload, publish, and copy a share link.
				</p>
			</div>
		</div>
	{/if}
</div>

{#if selectionMode}
	<SelectionBar
		count={selectedIds.size}
		folders={foldersStore.items}
		tags={tagsStore.items}
		onmove={bulkMove}
		onaddtag={bulkAddTag}
		ondelete={() => (confirmBulkDelete = true)}
		onclear={clearSelection}
	/>
{/if}

{#if renaming}
	<RenameDialog
		recast={renaming}
		onclose={() => (renaming = null)}
		onsave={(title) => renaming && doRename(renaming, title)}
	/>
{/if}

{#if managingTags}
	<TagManagerDialog onclose={() => (managingTags = false)} />
{/if}

<ConfirmDialog
	bind:open={() => confirmDelete !== null, (v) => !v && (confirmDelete = null)}
	title="Delete this recast?"
	description={`“${confirmDelete?.title ?? ""}”, its share links and everything viewers left on them go for good. Storage is freed.`}
	confirmLabel="Delete recast"
	busy={acting}
	onconfirm={() => confirmDelete && deleteRecast(confirmDelete)}
/>

<ConfirmDialog
	bind:open={confirmBulkDelete}
	title={`Delete ${selectedIds.size} recast${plural(selectedIds.size)}?`}
	description="Their share links and viewer activity go with them. This can't be undone."
	confirmLabel="Delete all"
	busy={acting}
	onconfirm={bulkDelete}
/>

<ConfirmDialog
	bind:open={() => confirmArchive !== null, (v) => !v && (confirmArchive = null)}
	title="Archive this recast?"
	description="The video file is deleted to free storage. Views and comments are kept, but playback stops until you upload it again."
	confirmLabel="Archive"
	destructive={false}
	busy={acting}
	onconfirm={() => confirmArchive && archiveRecast(confirmArchive)}
/>
