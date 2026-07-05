<script lang="ts">
	import { invalidateAll } from "$app/navigation";
	import * as api from "$lib/dashboard/api";
	import ArchivedCard, { type ArchivedRecast } from "$lib/dashboard/components/ArchivedCard.svelte";
	import EmptyState from "$lib/dashboard/components/EmptyState.svelte";
	import FolderRail, { type FolderSelection } from "$lib/dashboard/components/FolderRail.svelte";
	import LibraryToolbar from "$lib/dashboard/components/LibraryToolbar.svelte";
	import PageHeader from "$lib/dashboard/components/PageHeader.svelte";
	import PlayerDialog from "$lib/dashboard/components/PlayerDialog.svelte";
	import RecastGrid from "$lib/dashboard/components/RecastGrid.svelte";
	import RenameDialog from "$lib/dashboard/components/RenameDialog.svelte";
	import SelectionBar from "$lib/dashboard/components/SelectionBar.svelte";
	import TagManagerDialog from "$lib/dashboard/components/TagManagerDialog.svelte";
	import { mapRecastsForStore } from "$lib/dashboard/hydrate";
	import { foldersStore, tagsStore } from "$lib/dashboard/library.svelte";
	import {
		recastsStore,
		type Recast,
		type RecordingSource,
	} from "$lib/dashboard/store.svelte";
	import { POSTER_ACCEPT, replacePoster } from "$lib/dashboard/poster";
	import { filterAndSortRecasts, isFileDrag } from "$lib/dashboard/recasts-library.logic";
	import { UPLOAD_ACCEPT } from "$lib/dashboard/upload";
	import { createUploadController } from "$lib/dashboard/upload.svelte";
	import { Archive, FolderOpen, Library, LoaderCircle, Upload, UploadCloud } from "@lucide/svelte";
	import { Button } from "@recast/ui/button";
	import { toast } from "@recast/ui/sonner";
	import { untrack } from "svelte";
	import { flip } from "svelte/animate";
	import { cubicOut } from "svelte/easing";
	import { fly, scale, slide } from "svelte/transition";

	let { data } = $props();

	// Hydrate recasts + folders + tags from the server.
	$effect(() => {
		const mapped = mapRecastsForStore(data.recasts);
		const folders = data.folders;
		const tags = data.tags;
		untrack(() => {
			recastsStore.hydrate(mapped);
			foldersStore.hydrate(folders);
			tagsStore.hydrate(tags);
		});
	});

	const workspaceId = $derived(data.workspaceId);

	// Archived recasts live in their own tab. Keep a local copy so a delete can
	// drop the card optimistically; re-seed whenever the loader returns fresh data.
	let archived = $state<ArchivedRecast[]>([]);
	$effect(() => {
		const next = data.archived;
		untrack(() => (archived = next));
	});

	type View = "library" | "archived";
	let view = $state<View>("library");

	let query = $state("");
	let activeFilter = $state<RecordingSource | "all">("all");
	let sortKey = $state<string>("recent");
	let selectedFolder = $state<FolderSelection>("all");
	let selectedTagIds = $state<string[]>([]);

	let playing = $state<Recast | null>(null);
	let renaming = $state<Recast | null>(null);
	let managingTags = $state(false);

	// Bulk selection.
	let selectedIds = $state(new Set<string>());
	const selectionMode = $derived(selectedIds.size > 0);

	// Upload (shared by the header button, the empty-state button, the file
	// input, and drag-and-drop).
	let fileInput = $state<HTMLInputElement | null>(null);
	let dragDepth = $state(0);
	const isDraggingFile = $derived(dragDepth > 0);
	const upload = createUploadController({
		workspaceId: () => workspaceId,
		onRefresh: invalidateAll,
	});

	const visible = $derived(
		filterAndSortRecasts(recastsStore.items, {
			query,
			activeFilter,
			folder: selectedFolder,
			tagIds: selectedTagIds,
			sortKey,
		}),
	);

	const hasRecasts = $derived(recastsStore.items.length > 0);
	const filtersActive = $derived(
		query.trim() !== "" || activeFilter !== "all" || selectedFolder !== "all" || selectedTagIds.length > 0,
	);
	const folderCrumb = $derived(
		typeof selectedFolder === "string" && selectedFolder !== "all" && selectedFolder !== "root"
			? foldersStore.breadcrumb(selectedFolder)
			: [],
	);

	function clearFilters() {
		query = "";
		activeFilter = "all";
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
		if (file) upload.start(file);
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

	function toggleSource(rec: Recast) {
		const next: RecordingSource = rec.source === "cloud" ? "local" : "cloud";
		recastsStore.setSource(rec.id, next);
		toast.success(next === "cloud" ? "Uploaded to Cloudinary." : "Moved to local storage.");
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
		if (rec.source !== "cloud") {
			toast.error("Upload this recast to the cloud first to set a poster.");
			return;
		}
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
		if (playing?.id === rec.id) playing = null;
		try {
			await api.deleteRecast(rec.id);
			toast.success(`“${rec.title}” deleted.`);
		} catch (e) {
			recastsStore.hydrate(snapshot);
			toast.error((e as Error)?.message ?? "Couldn't delete recast.");
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
		if (playing && ids.includes(playing.id)) playing = null;
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
</script>

<svelte:head>
	<title>Recasts - Recast Dashboard</title>
</svelte:head>

<input bind:this={fileInput} type="file" accept={UPLOAD_ACCEPT} class="hidden" onchange={upload.onFilePicked} />
<input bind:this={posterInput} type="file" accept={POSTER_ACCEPT} class="hidden" onchange={onPosterPicked} />

<PageHeader icon={Library} title="Recasts" subtitle="Everything you've captured, uploaded, and shared.">
	<Button class="gap-2" disabled={upload.uploading} onclick={() => fileInput?.click()}>
		{#if upload.uploading}<LoaderCircle class="size-4 animate-spin" />{:else}<Upload class="size-4" />{/if}
		{upload.uploading ? upload.label : "Upload recast"}
	</Button>
</PageHeader>

<!-- Inline upload progress -->
{#if upload.uploading}
	<div class="mt-4" transition:slide={{ duration: 200, easing: cubicOut }}>
		<div class="flex items-center justify-between text-xs text-muted-foreground">
			<span class="font-medium text-foreground">{upload.label}</span>
			{#if upload.phase === "uploading"}<span class="font-mono tabular-nums">{upload.pct}%</span>{/if}
		</div>
		<div class="mt-2 h-1.5 overflow-hidden rounded-full bg-foreground/8">
			<div
				class="h-full rounded-full bg-linear-to-r from-primary/70 to-primary transition-[width] duration-300 ease-[cubic-bezier(0.625,0.05,0,1)]"
				style="width: {upload.phase === 'uploading' ? upload.pct : 100}%"
				class:animate-pulse={upload.phase !== "uploading"}
			></div>
		</div>
	</div>
{/if}

<!-- View tabs: Library / Archived -->
<div class="mt-8 flex items-center gap-1 border-b border-border-low/60" in:fly={{ y: 12, duration: 480, delay: 200, easing: cubicOut }}>
	<button
		type="button"
		onclick={() => (view = "library")}
		class="-mb-px flex items-center gap-1.5 border-b-2 px-3 py-2 text-sm font-semibold transition-colors
			{view === 'library' ? 'border-primary text-foreground' : 'border-transparent text-muted-foreground hover:text-foreground'}"
	>
		<Library class="size-4" />
		Library
	</button>
	<button
		type="button"
		onclick={() => (view = "archived")}
		class="-mb-px flex items-center gap-1.5 border-b-2 px-3 py-2 text-sm font-semibold transition-colors
			{view === 'archived' ? 'border-primary text-foreground' : 'border-transparent text-muted-foreground hover:text-foreground'}"
	>
		<Archive class="size-4" />
		Archived
		{#if archived.length > 0}
			<span class="rounded-full bg-foreground/10 px-1.5 py-0.5 font-mono text-[10px] tabular-nums text-muted-foreground">{archived.length}</span>
		{/if}
	</button>
</div>

{#if view === "library"}
	<!-- Library: folder rail + content. The whole region is a file drop target. -->
	<div
		role="region"
		aria-label="Recast library"
		class="relative mt-6 flex flex-col gap-6 lg:flex-row"
		in:fly={{ y: 12, duration: 480, delay: 80, easing: cubicOut }}
		ondragenter={onDragEnter}
		ondragover={onDragOver}
		ondragleave={onDragLeave}
		ondrop={onDrop}
	>
		<FolderRail
			{workspaceId}
			selected={selectedFolder}
			onselect={(s) => (selectedFolder = s)}
			onDropRecast={(recastId, folderId) => {
				const rec = recastsStore.items.find((r) => r.id === recastId);
				if (rec) moveRecast(rec, folderId);
			}}
		/>

		<div class="min-w-0 flex-1">
			<LibraryToolbar
				bind:query
				bind:activeFilter
				bind:sortKey
				bind:selectedTagIds
				total={recastsStore.items.length}
				shown={visible.length}
				{filtersActive}
				onclear={clearFilters}
				onmanagetags={() => (managingTags = true)}
				oncreatetag={createTag}
			/>

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
					hasAnyRecasts={hasRecasts}
					{filtersActive}
					uploading={upload.uploading}
					uploadLabel={upload.label}
					onplay={(rec) => (playing = rec)}
					onrename={(rec) => (renaming = rec)}
					oncopylink={copyLink}
					onchangeposter={changePoster}
					ontogglesource={toggleSource}
					onmove={moveRecast}
					ontoggletag={toggleTag}
					ondelete={deleteRecast}
					onToggleSelect={toggleSelect}
					onupload={() => fileInput?.click()}
					onclearfilters={clearFilters}
				/>
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
{:else}
	<!-- Archived tab -->
	<div class="mt-6" in:fly={{ y: 12, duration: 480, delay: 80, easing: cubicOut }}>
		{#if archived.length > 0}
			<p class="mb-5 max-w-2xl text-sm text-muted-foreground">
				These recasts lost their cloud file after 14 days without views, so only
				the details remain. Re-share from the Recast desktop app to bring one back,
				or delete it for good. Each is purged automatically 16 days after archiving.
			</p>
			<div class="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3">
				{#each archived as rec (rec.id)}
					<div
						animate:flip={{ duration: 320, easing: cubicOut }}
						in:scale={{ start: 0.97, duration: 300, easing: cubicOut }}
						out:scale={{ start: 0.97, duration: 170, easing: cubicOut }}
					>
						<ArchivedCard recast={rec} ondelete={() => deleteArchived(rec)} />
					</div>
				{/each}
			</div>
		{:else}
			<EmptyState
				icon={Archive}
				title="Nothing archived"
				description="Unwatched recasts on the Free plan are archived after 14 days. They'll show up here to restore or remove."
			/>
		{/if}
	</div>
{/if}

{#if view === "library" && selectionMode}
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

{#if playing}
	<PlayerDialog
		recast={playing}
		onclose={() => (playing = null)}
		onengagement={(event) => {
			if (event.type === "view-start" && playing) {
				recastsStore.incrementViews(playing.id);
			}
		}}
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
