<script lang="ts">
	import { createFolder, deleteFolder, updateFolder } from "$lib/dashboard/api";
	import { focusOnMount } from "$lib/dashboard/focus";
	import { foldersStore, type Folder } from "$lib/dashboard/library.svelte";
	import { recastsStore } from "$lib/dashboard/store.svelte";
	import * as DropdownMenu from "@recast/ui/dropdown-menu";
	import { toast } from "@recast/ui/sonner";
	import { cn } from "@recast/ui/utils";
	import {
		ChevronRight,
		FolderClosed,
		FolderOpen,
		FolderPlus,
		Inbox,
		Layers,
		MoreHorizontal,
		Pencil,
		Plus,
		Trash2,
	} from "@recast/icons";

	export type FolderSelection = "all" | "root" | string;

	let {
		workspaceId,
		selected,
		onselect,
		onDropRecast,
	}: {
		workspaceId: string;
		selected: FolderSelection;
		onselect: (sel: FolderSelection) => void;
		onDropRecast: (recastId: string, folderId: string | null) => void;
	} = $props();

	let expanded = $state<Set<string>>(new Set());
	let creatingParent = $state<string | null | undefined>(undefined); // undefined = not creating
	let draftName = $state("");
	let renamingId = $state<string | null>(null);
	let dropTarget = $state<FolderSelection | null>(null);

	function toggle(id: string) {
		const next = new Set(expanded);
		if (next.has(id)) next.delete(id);
		else next.add(id);
		expanded = next;
	}

	function countFor(sel: FolderSelection): number {
		if (sel === "all") return recastsStore.items.length;
		if (sel === "root") return recastsStore.items.filter((r) => !r.folderId).length;
		return recastsStore.items.filter((r) => r.folderId === sel).length;
	}

	function startCreate(parentId: string | null) {
		creatingParent = parentId;
		draftName = "";
		if (parentId) expanded = new Set(expanded).add(parentId);
	}

	async function commitCreate() {
		const name = draftName.trim();
		const parentId = creatingParent ?? null;
		creatingParent = undefined;
		draftName = "";
		if (!name) return;
		try {
			const folder = await createFolder({ workspaceId, name, parentId });
			foldersStore.add(folder);
			toast.success(`Folder “${name}” created.`);
		} catch (e) {
			toast.error((e as Error)?.message ?? "Couldn't create folder.");
		}
	}

	async function commitRename(folder: Folder) {
		const name = draftName.trim();
		renamingId = null;
		if (!name || name === folder.name) return;
		const prev = folder.name;
		foldersStore.update(folder.id, { name });
		try {
			await updateFolder(folder.id, { name });
		} catch (e) {
			foldersStore.update(folder.id, { name: prev });
			toast.error((e as Error)?.message ?? "Couldn't rename folder.");
		}
	}

	async function removeFolder(folder: Folder) {
		const ids = foldersStore.subtreeIds(folder.id);
		foldersStore.remove(folder.id);
		recastsStore.clearFolder(ids);
		if (selected !== "all" && ids.has(selected as string)) onselect("all");
		try {
			await deleteFolder(folder.id);
			toast.success(`Folder “${folder.name}” deleted.`);
		} catch (e) {
			toast.error((e as Error)?.message ?? "Couldn't delete folder.");
		}
	}

	function onDrop(e: DragEvent, folderId: string | null) {
		e.preventDefault();
		dropTarget = null;
		const id = e.dataTransfer?.getData("text/recast-id");
		if (id) onDropRecast(id, folderId);
	}

	function onCreateKey(e: KeyboardEvent) {
		if (e.key === "Enter") (e.currentTarget as HTMLInputElement).blur();
		if (e.key === "Escape") {
			creatingParent = undefined;
			draftName = "";
		}
	}
</script>

<aside class="w-full shrink-0 lg:w-56">
	<div class="flex items-center justify-between px-2 pb-2">
		<span class="text-[11px] font-semibold uppercase tracking-[0.14em] text-muted-foreground">Library</span>
		<button
			type="button"
			onclick={() => startCreate(null)}
			aria-label="New folder"
			class="grid size-6 place-items-center rounded-md text-muted-foreground transition-colors hover:bg-foreground/8 hover:text-foreground"
		>
			<FolderPlus class="size-3.5" />
		</button>
	</div>

	<nav class="flex flex-col gap-0.5 text-sm">
		<!-- All recasts -->
		<button
			type="button"
			onclick={() => onselect("all")}
			ondragover={(e) => {
				e.preventDefault();
				dropTarget = "all";
			}}
			ondragleave={() => (dropTarget = dropTarget === "all" ? null : dropTarget)}
			ondrop={(e) => onDrop(e, null)}
			class={cn(
				"flex items-center gap-2 rounded-lg px-2.5 py-1.5 text-left transition-colors",
				selected === "all" ? "bg-foreground/8 text-foreground" : "text-muted-foreground hover:bg-foreground/5 hover:text-foreground",
				dropTarget === "all" && "ring-1 ring-primary/50",
			)}
		>
			<Layers class="size-4 shrink-0" />
			<span class="flex-1 truncate">All recasts</span>
			<span class="font-mono text-[10px] tabular-nums text-muted-foreground">{countFor("all")}</span>
		</button>

		<!-- No folder (root) -->
		<button
			type="button"
			onclick={() => onselect("root")}
			ondragover={(e) => {
				e.preventDefault();
				dropTarget = "root";
			}}
			ondragleave={() => (dropTarget = dropTarget === "root" ? null : dropTarget)}
			ondrop={(e) => onDrop(e, null)}
			class={cn(
				"flex items-center gap-2 rounded-lg px-2.5 py-1.5 text-left transition-colors",
				selected === "root" ? "bg-foreground/8 text-foreground" : "text-muted-foreground hover:bg-foreground/5 hover:text-foreground",
				dropTarget === "root" && "ring-1 ring-primary/50",
			)}
		>
			<Inbox class="size-4 shrink-0" />
			<span class="flex-1 truncate">No folder</span>
			<span class="font-mono text-[10px] tabular-nums text-muted-foreground">{countFor("root")}</span>
		</button>

		<div class="my-1 h-px bg-border-low/40"></div>

		{#snippet folderNode(folder: Folder, depth: number)}
			{@const kids = foldersStore.childrenOf(folder.id)}
			{@const isOpen = expanded.has(folder.id)}
			{@const active = selected === folder.id}
			{#if renamingId === folder.id}
				<input
					value={folder.name}
					oninput={(e) => (draftName = e.currentTarget.value)}
					onblur={() => commitRename(folder)}
					onkeydown={(e) => {
						if (e.key === "Enter") e.currentTarget.blur();
						if (e.key === "Escape") renamingId = null;
					}}
					class="w-full rounded-lg border border-primary/50 bg-background px-2.5 py-1.5 text-sm outline-none"
					style="margin-left: {depth * 12}px"
					use:focusOnMount
				/>
			{:else}
				<!-- Flex container (NOT a button) so chevron / label / menu can
				     each be their own button without illegal nesting. The whole
				     row is the drop target. -->
				<div
					role="presentation"
					ondragover={(e) => {
						e.preventDefault();
						dropTarget = folder.id;
					}}
					ondragleave={() => (dropTarget = dropTarget === folder.id ? null : dropTarget)}
					ondrop={(e) => onDrop(e, folder.id)}
					class={cn(
						"group/row flex items-center gap-1 rounded-lg pr-1 transition-colors",
						active ? "bg-foreground/8 text-foreground" : "text-muted-foreground hover:bg-foreground/5",
						dropTarget === folder.id && "ring-1 ring-primary/50",
					)}
					style="padding-left: {depth * 12 + 6}px"
				>
					{#if kids.length > 0}
						<button
							type="button"
							onclick={() => toggle(folder.id)}
							aria-label={isOpen ? "Collapse" : "Expand"}
							class="grid size-4 shrink-0 place-items-center rounded text-muted-foreground/70 hover:text-foreground"
						>
							<ChevronRight class={cn("size-3.5 transition-transform", isOpen && "rotate-90")} />
						</button>
					{:else}
						<span class="size-4 shrink-0"></span>
					{/if}
					<button
						type="button"
						onclick={() => onselect(folder.id)}
						class="flex min-w-0 flex-1 items-center gap-1.5 py-1.5 text-left hover:text-foreground"
					>
						{#if folder.color}
							<span class="size-2.5 shrink-0 rounded-[3px]" style="background:{folder.color}"></span>
						{:else if isOpen}
							<FolderOpen class="size-4 shrink-0 text-muted-foreground" />
						{:else}
							<FolderClosed class="size-4 shrink-0 text-muted-foreground" />
						{/if}
						<span class="truncate">{folder.name}</span>
					</button>
					<span class="font-mono text-[10px] tabular-nums text-muted-foreground group-hover/row:hidden">{countFor(folder.id)}</span>
					<DropdownMenu.Root>
						<DropdownMenu.Trigger
							class="hidden size-6 shrink-0 place-items-center rounded-md text-muted-foreground hover:bg-foreground/10 hover:text-foreground group-hover/row:grid"
							aria-label="Folder options"
						>
							<MoreHorizontal class="size-3.5" />
						</DropdownMenu.Trigger>
						<DropdownMenu.Content align="end" sideOffset={4} class="w-44">
							<DropdownMenu.Item
								onclick={() => {
									renamingId = folder.id;
									draftName = folder.name;
								}}
							>
								<Pencil class="size-3.5 text-muted-foreground" /> Rename
							</DropdownMenu.Item>
							<DropdownMenu.Item onclick={() => startCreate(folder.id)}>
								<Plus class="size-3.5 text-muted-foreground" /> New subfolder
							</DropdownMenu.Item>
							<DropdownMenu.Separator />
							<DropdownMenu.Item
								onclick={() => removeFolder(folder)}
								class="text-destructive/90 data-highlighted:text-destructive"
							>
								<Trash2 class="size-3.5" /> Delete
							</DropdownMenu.Item>
						</DropdownMenu.Content>
					</DropdownMenu.Root>
				</div>
			{/if}

			{#if isOpen}
				{#each kids as child (child.id)}
					{@render folderNode(child, depth + 1)}
				{/each}
			{/if}

			{#if creatingParent === folder.id}
				<input
					bind:value={draftName}
					onblur={commitCreate}
					onkeydown={onCreateKey}
					placeholder="Folder name"
					class="mt-0.5 w-full rounded-lg border border-primary/50 bg-background px-2.5 py-1.5 text-sm outline-none placeholder:text-muted-foreground/60"
					style="margin-left: {(depth + 1) * 12}px"
					use:focusOnMount
				/>
			{/if}
		{/snippet}

		{#each foldersStore.childrenOf(null) as root (root.id)}
			{@render folderNode(root, 0)}
		{/each}

		{#if creatingParent === null}
			<input
				bind:value={draftName}
				onblur={commitCreate}
				onkeydown={onCreateKey}
				placeholder="Folder name"
				class="mt-0.5 w-full rounded-lg border border-primary/50 bg-background px-2.5 py-1.5 text-sm outline-none placeholder:text-muted-foreground/60"
				use:focusOnMount
			/>
		{/if}

		{#if foldersStore.items.length === 0 && creatingParent === undefined}
			<button
				type="button"
				onclick={() => startCreate(null)}
				class="mt-1 flex items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-left text-xs text-muted-foreground transition-colors hover:bg-foreground/5 hover:text-foreground"
			>
				<FolderPlus class="size-3.5" /> New folder
			</button>
		{/if}
	</nav>

	<p class="mt-3 px-2.5 text-[10px] leading-relaxed text-muted-foreground/70">
		Drag a recast onto a folder to file it.
	</p>
</aside>
