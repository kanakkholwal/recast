<script lang="ts">
	import type { Folder, Tag } from "$lib/dashboard/library.svelte";
	import { Button } from "@recast/ui/button";
	import * as DropdownMenu from "@recast/ui/dropdown-menu";
	import { FolderInput, Inbox, Tag as TagIcon, Trash2, X } from "@recast/icons";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";

	// Floating bulk-action bar shown while one or more recasts are selected.
	// Batches the same move / tag / delete mutations the per-card menu exposes.
	let {
		count,
		folders,
		tags,
		onmove,
		onaddtag,
		ondelete,
		onclear,
	}: {
		count: number;
		folders: Folder[];
		tags: Tag[];
		onmove: (folderId: string | null) => void;
		onaddtag: (tagId: string) => void;
		ondelete: () => void;
		onclear: () => void;
	} = $props();

	const sortedFolders = $derived([...folders].sort((a, b) => a.path.localeCompare(b.path)));
	function depthOf(path: string): number {
		return Math.max(0, (path.match(/\//g)?.length ?? 1) - 2);
	}

	const triggerClass =
		"inline-flex items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-xs font-semibold text-foreground outline-none transition-colors hover:bg-foreground/8 focus-visible:ring-2 focus-visible:ring-ring/50";
</script>

<div
	class="pointer-events-none fixed inset-x-0 bottom-6 z-40 flex justify-center px-4"
	transition:fly={{ y: 16, duration: 220, easing: cubicOut }}
>
	<div class="glass-strong pointer-events-auto flex items-center gap-1 rounded-2xl border border-border-low/60 p-1.5 pl-3 shadow-craft-floating">
		<span class="mr-1.5 text-xs font-semibold tabular-nums text-foreground">{count} selected</span>

		<!-- Move to folder -->
		<DropdownMenu.Root>
			<DropdownMenu.Trigger class={triggerClass}>
				<FolderInput class="size-3.5 text-muted-foreground" />
				Move
			</DropdownMenu.Trigger>
			<DropdownMenu.Content side="top" sideOffset={8} align="center" class="max-h-72 w-56 overflow-y-auto">
				<DropdownMenu.Item onclick={() => onmove(null)}>
					<Inbox class="size-4 text-muted-foreground" />
					No folder
				</DropdownMenu.Item>
				{#if sortedFolders.length > 0}
					<DropdownMenu.Separator />
					{#each sortedFolders as f (f.id)}
						<DropdownMenu.Item onclick={() => onmove(f.id)}>
							<span style="width: {depthOf(f.path) * 10}px" class="shrink-0"></span>
							{#if f.color}
								<span class="size-2.5 shrink-0 rounded-[3px]" style="background:{f.color}"></span>
							{/if}
							<span class="flex-1 truncate">{f.name}</span>
						</DropdownMenu.Item>
					{/each}
				{/if}
			</DropdownMenu.Content>
		</DropdownMenu.Root>

		<!-- Add tag -->
		<DropdownMenu.Root>
			<DropdownMenu.Trigger class={triggerClass}>
				<TagIcon class="size-3.5 text-muted-foreground" />
				Tag
			</DropdownMenu.Trigger>
			<DropdownMenu.Content side="top" sideOffset={8} align="center" class="max-h-72 w-56 overflow-y-auto">
				{#if tags.length === 0}
					<div class="px-2 py-2 text-xs text-muted-foreground">No tags yet.</div>
				{:else}
					{#each tags as t (t.id)}
						<DropdownMenu.Item onclick={() => onaddtag(t.id)}>
							{#if t.color}
								<span class="size-2.5 shrink-0 rounded-full" style="background:{t.color}"></span>
							{/if}
							{t.name}
						</DropdownMenu.Item>
					{/each}
				{/if}
			</DropdownMenu.Content>
		</DropdownMenu.Root>

		<button type="button" onclick={ondelete} class="{triggerClass} text-destructive hover:bg-destructive/10">
			<Trash2 class="size-3.5" />
			Delete
		</button>

		<span class="mx-0.5 h-5 w-px bg-border-low/60"></span>

		<Button variant="ghost" size="icon" class="size-8" aria-label="Clear selection" onclick={onclear}>
			<X class="size-4" />
		</Button>
	</div>
</div>
