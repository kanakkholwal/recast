<script lang="ts">
	import type { Folder, Tag } from "$lib/dashboard/library.svelte";
	import type { Recast } from "$lib/dashboard/store.svelte";
	import {
		formatBytes,
		formatCount,
		formatDuration,
		formatRelative,
	} from "$lib/dashboard/format";
	import EmptyState from "./EmptyState.svelte";
	import RecastCard from "./RecastCard.svelte";
	import * as DropdownMenu from "@recast/ui/dropdown-menu";
	import { Button } from "@recast/ui/button";
	import {
		Archive,
		BarChart3,
		Check,
		Film,
		Link2,
		MoreHorizontal,
		Pencil,
		Play,
		Trash2,
		Upload,
	} from "@recast/icons";
	import { goto } from "$app/navigation";
	import { flip } from "svelte/animate";
	import { cubicOut } from "svelte/easing";
	import { scale } from "svelte/transition";

	// Responsive recast grid + its three empty states (no recasts / no match /
	// empty folder). Extracted from the library page so the page keeps only the
	// data + handlers.
	let {
		recasts,
		folders,
		tags,
		selectedIds = new Set<string>(),
		selectionMode = false,
		viewMode = "grid",
		hasAnyRecasts,
		filtersActive,
		onrename,
		oncopylink,
		onchangeposter,
		onmove,
		ontoggletag,
		onarchive,
		ondelete,
		onToggleSelect,
		onupload,
		onclearfilters,
	}: {
		recasts: Recast[];
		folders: Folder[];
		tags: Tag[];
		selectedIds?: Set<string>;
		selectionMode?: boolean;
		viewMode?: "grid" | "list";
		hasAnyRecasts: boolean;
		filtersActive: boolean;
		onrename: (rec: Recast) => void;
		oncopylink: (rec: Recast) => void;
		onchangeposter?: (rec: Recast) => void;
		onmove: (rec: Recast, folderId: string | null) => void;
		ontoggletag: (rec: Recast, tagId: string) => void;
		onarchive?: (rec: Recast) => void;
		ondelete: (rec: Recast) => void;
		onToggleSelect: (rec: Recast) => void;
		onupload: () => void;
		onclearfilters: () => void;
} = $props();
</script>

{#if recasts.length > 0}
	{#if viewMode === "grid"}
		<div class="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4">
			{#each recasts as rec (rec.id)}
				<div
					animate:flip={{ duration: 320, easing: cubicOut }}
					in:scale={{ start: 0.97, duration: 300, easing: cubicOut }}
					out:scale={{ start: 0.97, duration: 170, easing: cubicOut }}
				>
					<RecastCard
						recast={rec}
						{folders}
						{tags}
						selectable
						selected={selectedIds.has(rec.id)}
						{selectionMode}
						onToggleSelect={() => onToggleSelect(rec)}
						onrename={() => onrename(rec)}
						oncopylink={() => oncopylink(rec)}
						onchangeposter={onchangeposter ? () => onchangeposter(rec) : undefined}
						onmove={(folderId) => onmove(rec, folderId)}
						ontoggletag={(tagId) => ontoggletag(rec, tagId)}
						onarchive={onarchive ? () => onarchive(rec) : undefined}
						ondelete={() => ondelete(rec)}
					/>
				</div>
			{/each}
		</div>
	{:else}
		<div class="overflow-hidden rounded-xl border border-border-low/70 bg-background/55">
			<div class="grid grid-cols-[2rem_minmax(0,1fr)_7rem_5rem_3rem] items-center border-b border-border-low/60 px-3 py-2 text-xs font-medium text-muted-foreground md:grid-cols-[2rem_minmax(0,1fr)_7rem_6rem_5rem_3rem]">
				<span></span>
				<span>Details</span>
				<span>Published</span>
				<span class="hidden md:block">Size</span>
				<span>Views</span>
				<span></span>
			</div>
			{#each recasts as rec (rec.id)}
				{@const selected = selectedIds.has(rec.id)}
				<div
					animate:flip={{ duration: 320, easing: cubicOut }}
					class="grid min-h-16 grid-cols-[2rem_minmax(0,1fr)_7rem_5rem_3rem] items-center border-b border-border-low/50 px-3 py-2 last:border-b-0 md:grid-cols-[2rem_minmax(0,1fr)_7rem_6rem_5rem_3rem]"
				>
					<button
						type="button"
						onclick={() => onToggleSelect(rec)}
						aria-label={selected ? "Deselect recast" : "Select recast"}
						aria-pressed={selected}
						class="grid size-5 place-items-center rounded border transition-colors {selected ? 'border-primary bg-primary text-background' : 'border-border-low bg-background text-transparent hover:border-foreground/40'}"
					>
						<Check class="size-3" />
					</button>
					<a
						href="/dashboard/recasts/{rec.id}"
						onclick={(e) => {
							if (selectionMode) {
								e.preventDefault();
								onToggleSelect(rec);
							}
						}}
						class="flex min-w-0 items-center gap-3 text-left"
					>
						<span class="relative h-11 w-16 shrink-0 overflow-hidden rounded-md bg-foreground/8">
							{#if rec.posterUrl}
								<img src={rec.posterUrl} alt="" loading="lazy" class="h-full w-full object-cover" />
							{:else}
								<span class="grid h-full w-full place-items-center">
									<Film class="size-4 text-muted-foreground" />
								</span>
							{/if}
							<span class="absolute inset-0 grid place-items-center bg-background/25 opacity-0 transition-opacity hover:opacity-100">
								<Play class="size-4 fill-current text-foreground" />
							</span>
						</span>
						<span class="min-w-0">
							<span class="block truncate text-sm font-semibold text-foreground">{rec.title}</span>
							<span class="mt-0.5 block truncate text-xs text-muted-foreground">
								{formatDuration(rec.durationSec)} · {formatBytes(rec.sizeBytes)}
							</span>
						</span>
					</a>
					<span class="text-sm text-muted-foreground">{formatRelative(rec.createdAt)}</span>
					<span class="hidden text-sm text-muted-foreground md:block">{formatBytes(rec.sizeBytes)}</span>
					<span class="font-mono text-sm tabular-nums text-foreground">{formatCount(rec.views)}</span>
					<DropdownMenu.Root>
						<DropdownMenu.Trigger
							class="grid size-8 place-items-center rounded-md text-muted-foreground transition-colors hover:bg-foreground/8 hover:text-foreground"
							aria-label="Recast options"
						>
							<MoreHorizontal class="size-4" />
						</DropdownMenu.Trigger>
						<DropdownMenu.Content align="end" sideOffset={6} class="w-48">
							<DropdownMenu.Item onclick={() => goto(`/dashboard/recasts/${rec.id}`)}>
								<Play class="size-4 text-muted-foreground" /> Open
							</DropdownMenu.Item>
							<DropdownMenu.Item onclick={() => onrename(rec)}>
								<Pencil class="size-4 text-muted-foreground" /> Rename
							</DropdownMenu.Item>
							<DropdownMenu.Item onclick={() => oncopylink(rec)}>
								<Link2 class="size-4 text-muted-foreground" /> Copy link
							</DropdownMenu.Item>
							<DropdownMenu.Item onclick={() => goto(`/dashboard/recasts/${rec.id}/analytics`)}>
								<BarChart3 class="size-4 text-muted-foreground" /> View analytics
							</DropdownMenu.Item>
							<DropdownMenu.Separator />
							{#if onarchive}
								<DropdownMenu.Item onclick={() => onarchive(rec)}>
									<Archive class="size-4 text-muted-foreground" /> Archive
								</DropdownMenu.Item>
							{/if}
							<DropdownMenu.Item
								onclick={() => ondelete(rec)}
								class="text-destructive/90 data-highlighted:text-destructive"
							>
								<Trash2 class="size-4" /> Delete
							</DropdownMenu.Item>
						</DropdownMenu.Content>
					</DropdownMenu.Root>
				</div>
			{/each}
		</div>
	{/if}
{:else if !hasAnyRecasts}
	<EmptyState icon={Film} title="No recasts yet" description="Upload an MP4, or capture and export one with the Recast desktop app.">
		<Button size="sm" class="gap-2" onclick={onupload}>
			<Upload class="size-3.5" />
			Upload recast
		</Button>
	</EmptyState>
{:else if filtersActive}
	<EmptyState icon={Film} title="No recasts match" description="Nothing here matches your search, folder, and tag filters.">
		<Button variant="outline" size="sm" onclick={onclearfilters}>Clear filters</Button>
	</EmptyState>
{:else}
	<EmptyState icon={Film} title="This folder is empty" description="Drag a recast onto it, or use “Move to” from a recast's menu." />
{/if}
