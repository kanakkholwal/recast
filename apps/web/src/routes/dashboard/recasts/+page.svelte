<script lang="ts">
	import { invalidateAll } from "$app/navigation";
	import * as api from "$lib/dashboard/api";
	import type { ArchivedRecast } from "$lib/dashboard/components/ArchivedCard.svelte";
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
	import { recastsStore, type Recast } from "$lib/dashboard/store.svelte";
	import {
	  Folder,
	  FolderOpen,
	  FolderPlus,
	  Grid2X2,
	  Library,
	  List,
	  MoreHorizontal,
	  Pencil,
	  Plus,
	  Trash2,
	  Upload,
	  UploadCloud,
	} from "@lucide/svelte";
	import { Button } from "@recast/ui/button";
	import * as DropdownMenu from "@recast/ui/dropdown-menu";
	import { toast } from "@recast/ui/sonner";
	import { cn } from "@recast/ui/utils";
	import { untrack } from "svelte";
	import { cubicOut } from "svelte/easing";
	import { fly, slide } from "svelte/transition";

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
	let archived = $state<ArchivedRecast[]>([]);

	let query = $state("");
	let sortKey = $state<string>("recent");
	let selectedFolder = $state<FolderSelection>("all");
	let selectedTagIds = $state<string[]>([]);

	let renaming = $state<Recast | null>(null);
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

	const visible = $derived(
		filterAndSortRecasts(recastsStore.items, {
			query,
			activeFilter: "all",
			folder: selectedFolder,
			tagIds: selectedTagIds,
			sortKey,
		}),
	);

	const hasRecasts = $derived(recastsStore.items.length > 0);
	const filtersActive = $derived(
		query.trim() !== "" || selectedFolder !== "all" || selectedTagIds.length > 0,
	);
	const folderCrumb = $derived(
		typeof selectedFolder === "string" && selectedFolder !== "all" && selectedFolder !== "root"
			? foldersStore.breadcrumb(selectedFolder)
			: [],
	);
	const folderCards = $derived([...foldersStore.items].sort((a, b) => a.path.localeCompare(b.path)));
	const selectedFolderName = $derived(
		selectedFolder === "all"
			? "All videos"
			: selectedFolder === "root"
				? "No folder"
				: foldersStore.get(selectedFolder)?.name ?? "Folder",
	);
	const libraryStats = $derived({
		folders: foldersStore.items.length,
		videos: recastsStore.items.length,
	});

	function countForFolder(folderId: string): number {
		const ids = foldersStore.subtreeIds(folderId);
		return recastsStore.items.filter((r) => r.folderId && ids.has(r.folderId)).length;
	}

	function clearFilters() {
		query = "";
		selectedFolder = "all";
		selectedTagIds = [];
	}

	// ── Selection ──────────────────────────────────────────────────────
	function toggleSelect(rec: Recast) {
		const next = new Set(selectedIds);
		if (next.has(rec.id)) next.delete(rec.id);
		else next.add(rec.id);
		selectedIds = next;
	}
	function clearSelection() {
		selectedIds = new Set();
	}

	// ── Upload (drag-and-drop; button/input flows go through `upload`) ──
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

	// ── Mutations ──────────────────────────────────────────────────────
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
			const name = folderId ? foldersStore.get(folderId)?.name ?? "folder" : "No folder";
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

	// ── Replace poster (cloud recasts only) ─────────────────────────────
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
		const snapshot = recastsStore.items;
		recastsStore.remove(rec.id);
		try {
			await api.deleteRecast(rec.id);
			toast.success(`“${rec.title}” deleted.`);
		} catch (e) {
			recastsStore.hydrate(snapshot);
			toast.error((e as Error)?.message ?? "Couldn't delete recast.");
		}
	}

	async function archiveRecast(rec: Recast) {
		const snapshot = recastsStore.items;
		recastsStore.remove(rec.id);
		try {
			await api.archiveRecast(rec.id);
			toast.success(`“${rec.title}” archived — storage freed.`);
			// Refresh the archived rail + quota usage below.
			void invalidateAll();
		} catch (e) {
			recastsStore.hydrate(snapshot);
			toast.error((e as Error)?.message ?? "Couldn't archive recast.");
		}
	}

	// ── Bulk mutations ─────────────────────────────────────────────────
	function plural(n: number) {
		return n === 1 ? "" : "s";
	}

	async function bulkMove(folderId: string | null) {
		const ids = [...selectedIds];
		const snapshot = recastsStore.items;
		ids.forEach((id) => recastsStore.move(id, folderId));
		clearSelection();
		try {
			await Promise.all(ids.map((id) => api.moveRecast(id, folderId)));
			const name = folderId ? foldersStore.get(folderId)?.name ?? "folder" : "No folder";
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
			const next = rec && !rec.tags.includes(tagId) ? [...rec.tags, tagId] : rec?.tags ?? [];
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
		const ids = [...selectedIds];
		const snapshot = recastsStore.items;
		ids.forEach((id) => recastsStore.remove(id));
		clearSelection();
		try {
			await Promise.all(ids.map((id) => api.deleteRecast(id)));
			toast.success(`Deleted ${ids.length} recast${plural(ids.length)}.`);
		} catch (e) {
			recastsStore.hydrate(snapshot);
			toast.error((e as Error)?.message ?? "Couldn't delete recasts.");
		}
	}

	async function deleteArchived(rec: ArchivedRecast) {
		const snapshot = archived;
		archived = archived.filter((a) => a.id !== rec.id);
		try {
			await api.deleteRecast(rec.id);
			toast.success(`“${rec.title}” deleted permanently.`);
		} catch (e) {
			archived = snapshot;
			toast.error((e as Error)?.message ?? "Couldn't delete recast.");
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

	async function createRootFolder() {
		const name = newFolderName.trim();
		creatingFolder = false;
		newFolderName = "";
		if (!name) return;
		try {
			const folder = await api.createFolder({ workspaceId, name, parentId: null });
			foldersStore.add(folder);
			selectedFolder = folder.id;
			toast.success(`Folder "${name}" created.`);
		} catch (e) {
			toast.error((e as Error)?.message ?? "Couldn't create folder.");
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
			selectedFolder = folder.id;
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

	async function commitFolderDraft(folderId?: string) {
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

<input bind:this={posterInput} type="file" accept={POSTER_ACCEPT} class="hidden" onchange={onPosterPicked} />

<PageHeader icon={Library} title="Recasts" subtitle="Everything you've captured, uploaded, and shared.">
	<Button class="gap-2" onclick={() => quickUpload.show()}>
		<Upload class="size-4" />
		Upload recast
	</Button>
</PageHeader>

	<!-- Library: folder cards + content. The whole region is a file drop target. -->
	<div
		role="region"
		aria-label="Recast library"
		class="glass-card shadow-none relative mt-8 overflow-hidden rounded-2xl p-4 sm:p-5"
		in:fly={{ y: 12, duration: 480, delay: 80, easing: cubicOut }}
		ondragenter={onDragEnter}
		ondragover={onDragOver}
		ondragleave={onDragLeave}
		ondrop={onDrop}
	>
		<div class="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
			<div>
				<h2 class="text-xl font-semibold tracking-tight text-foreground">Videos</h2>
				<p class="mt-1 text-sm text-muted-foreground">
					{libraryStats.videos} videos · {libraryStats.folders} folders
				</p>
			</div>
			<div class="flex flex-wrap items-center gap-2">
				<Button variant="outline" size="sm" class="gap-2" onclick={() => startCreateFolder()}>
					<Plus class="size-3.5" />
					New folder
				</Button>
				<div class="grid h-9 grid-cols-2 rounded-lg border border-border-low/70 bg-background/50 p-0.5">
					<button
						type="button"
						onclick={() => (viewMode = "list")}
						aria-pressed={viewMode === "list"}
						class={cn(
							"inline-flex min-w-24 items-center justify-center gap-2 rounded-md px-3 text-sm font-medium transition-colors",
							viewMode === "list" ? "bg-card text-foreground shadow-craft-sm" : "text-muted-foreground hover:text-foreground",
						)}
					>
						<List class="size-3.5" />
						List
					</button>
					<button
						type="button"
						onclick={() => (viewMode = "grid")}
						aria-pressed={viewMode === "grid"}
						class={cn(
							"inline-flex min-w-24 items-center justify-center gap-2 rounded-md px-3 text-sm font-medium transition-colors",
							viewMode === "grid" ? "bg-card text-foreground shadow-craft-sm" : "text-muted-foreground hover:text-foreground",
						)}
					>
						<Grid2X2 class="size-3.5" />
						Grid
					</button>
				</div>
			</div>
		</div>

		<div class="mt-6">
			<div class="mb-3 flex items-center justify-between gap-3">
				<h3 class="text-sm font-semibold text-foreground">Folders</h3>
				<span class="font-mono text-xs tabular-nums text-muted-foreground">{folderCards.length}</span>
			</div>
			<div class="grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
				<button
					type="button"
					onclick={() => (selectedFolder = "all")}
					class={cn(
						"flex min-h-16 items-center gap-3 rounded-lg border px-3 text-left transition-colors",
						selectedFolder === "all"
							? "border-primary/35 bg-primary/8"
							: "border-border-low/70 bg-background/45 hover:border-primary/30 hover:bg-background/70",
					)}
				>
					<span class="grid size-9 place-items-center rounded-md bg-foreground/5 text-muted-foreground">
						<Library class="size-4" />
					</span>
					<span class="min-w-0">
						<span class="block truncate text-sm font-semibold text-foreground">All videos</span>
						<span class="text-xs text-muted-foreground">{recastsStore.items.length} videos</span>
					</span>
				</button>
				{#each folderCards as folder (folder.id)}
					{@const isRenaming = renamingFolderId === folder.id}
					<div
						role="listitem"
						class={cn(
							"group/folder flex min-h-16 items-center gap-3 rounded-lg border px-3 text-left transition-colors",
							selectedFolder === folder.id
								? "border-primary/35 bg-primary/8"
								: "border-border-low/70 bg-background/45 hover:border-primary/30 hover:bg-background/70",
						)}
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
							class="flex min-w-0 flex-1 items-center gap-3 text-left"
						>
							<span class="grid size-9 shrink-0 place-items-center rounded-md bg-foreground/5 text-muted-foreground">
								{#if folder.color}
									<span class="size-4 rounded-[4px]" style="background:{folder.color}"></span>
								{:else}
									<Folder class="size-4" />
								{/if}
							</span>
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
										class="block w-full bg-transparent text-sm font-semibold text-foreground outline-none"
									/>
								{:else}
									<span class="block truncate text-sm font-semibold text-foreground">{folder.name}</span>
								{/if}
								<span class="text-xs text-muted-foreground">{countForFolder(folder.id)} videos</span>
							</span>
						</button>
						<DropdownMenu.Root>
							<DropdownMenu.Trigger
								class="grid size-8 shrink-0 place-items-center rounded-md text-muted-foreground opacity-100 transition-colors hover:bg-foreground/8 hover:text-foreground sm:opacity-0 sm:group-hover/folder:opacity-100"
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
						class="flex min-h-16 items-center gap-3 rounded-lg border border-primary/35 bg-primary/8 px-3"
						onsubmit={(e) => {
							e.preventDefault();
							createFolder();
						}}
					>
						<span class="grid size-9 shrink-0 place-items-center rounded-md bg-background/70 text-primary">
							<Folder class="size-4" />
						</span>
						<input
							bind:value={newFolderName}
							placeholder="Folder name"
							onkeydown={(e) => {
								if (e.key === "Escape") {
									cancelFolderDraft();
								}
							}}
							onblur={() => commitFolderDraft()}
							class="min-w-0 flex-1 bg-transparent text-sm font-semibold text-foreground outline-none placeholder:text-muted-foreground/70"
						/>
					</form>
				{/if}
			</div>
		</div>

		<div class="mt-6">
			<div class="min-w-0">
				<div class="mb-4 flex flex-col gap-3 lg:flex-row lg:items-end lg:justify-between">
					<div>
						<h3 class="text-sm font-semibold text-foreground">{selectedFolderName}</h3>
						<p class="mt-0.5 text-xs text-muted-foreground">
							{visible.length} matching {visible.length === 1 ? "video" : "videos"}
						</p>
					</div>
					<div class="lg:min-w-[min(100%,42rem)]">
						<LibraryToolbar
							bind:query
							bind:sortKey
							bind:selectedTagIds
							total={recastsStore.items.length}
							shown={visible.length}
							{filtersActive}
							onclear={clearFilters}
							onmanagetags={() => (managingTags = true)}
							oncreatetag={createTag}
						/>
					</div>
				</div>

			<!-- Folder context line -->
			{#if folderCrumb.length > 0}
				<div class="mt-4 flex items-center gap-1.5 text-sm text-muted-foreground" in:slide={{ duration: 200, easing: cubicOut }}>
					<FolderOpen class="size-4 text-primary" />
					{#each folderCrumb as f, i (f.id)}
						<button type="button" onclick={() => (selectedFolder = f.id)} class="transition-colors hover:text-foreground {i === folderCrumb.length - 1 ? 'font-medium text-foreground' : ''}">
							{f.name}
						</button>
						{#if i < folderCrumb.length - 1}<span class="text-muted-foreground/50">/</span>{/if}
					{/each}
				</div>
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
					onarchive={archiveRecast}
					ondelete={deleteRecast}
					onToggleSelect={toggleSelect}
					onupload={() => quickUpload.show()}
					onclearfilters={clearFilters}
				/>
			</div>
		</div>
		</div>

		<!-- Drop-to-upload overlay -->
		{#if isDraggingFile}
			<div
				class="pointer-events-none absolute inset-0 z-30 grid place-items-center rounded-2xl border-2 border-dashed border-primary/60 bg-background/70 backdrop-blur-sm"
				transition:fly={{ y: 8, duration: 160, easing: cubicOut }}
			>
				<div class="flex flex-col items-center gap-2 text-center">
					<span class="glass-chip grid size-12 place-items-center rounded-xl text-primary">
						<UploadCloud class="size-5" />
					</span>
					<p class="text-sm font-semibold text-foreground">Drop to upload</p>
					<p class="text-xs text-muted-foreground">We'll upload, publish, and copy a share link.</p>
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
		ondelete={bulkDelete}
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
