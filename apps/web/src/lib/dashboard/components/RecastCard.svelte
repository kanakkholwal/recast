<script lang="ts">
import {
	Archive,
	BarChart3,
	Check,
	Eye,
	Film,
	FolderInput,
	ImagePlus,
	Inbox,
	Link2,
	MoreHorizontal,
	Pencil,
	Play,
	Share2,
	Tag as TagIcon,
	Trash2,
} from "@recast/icons";
import { Chip } from "@recast/ui/chip";
import * as DropdownMenu from "@recast/ui/dropdown-menu";
import { goto } from "$app/navigation";
import { formatBytes, formatCount, formatDuration, formatRelative } from "$lib/dashboard/format";
import type { Folder, Tag } from "$lib/dashboard/library.svelte";
import type { Recast } from "$lib/dashboard/store.svelte";
import { folderDepth, resolveAssignedTags, sortFoldersByPath } from "./RecastCard.logic";

let {
	recast,
	folders,
	tags,
	selectable = false,
	selected = false,
	selectionMode = false,
	onToggleSelect,
	onrename,
	oncopylink,
	onchangeposter,
	onmove,
	ontoggletag,
	onarchive,
	ondelete,
}: {
	recast: Recast;
	folders: Folder[];
	tags: Tag[];
	/** Show the selection checkbox (on hover / when in selection mode). */
	selectable?: boolean;
	selected?: boolean;
	/** When any card is selected, clicking a card toggles it instead of playing. */
	selectionMode?: boolean;
	onToggleSelect?: () => void;
	onrename: () => void;
	oncopylink: () => void;
	onchangeposter?: () => void;
	onmove: (folderId: string | null) => void;
	ontoggletag: (tagId: string) => void;
	onarchive?: () => void;
	ondelete: () => void;
} = $props();

const isShared = $derived(!!recast.latestShareSlug);
const showViews = $derived(recast.source === "cloud" && recast.views > 0);

let posterFailed = $state(false);
const showPoster = $derived(!!recast.posterUrl && !posterFailed);

const assignedTags = $derived(resolveAssignedTags(recast.tags, tags));
const assignedSet = $derived(new Set(recast.tags));

const sortedFolders = $derived(sortFoldersByPath(folders));

function onDragStart(e: DragEvent) {
	e.dataTransfer?.setData("text/recast-id", recast.id);
	if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
}
</script>

<article
	draggable="true"
	ondragstart={onDragStart}
	class="surface group/card relative flex h-full cursor-grab flex-col overflow-hidden transition-colors active:cursor-grabbing motion-reduce:transition-none
		{selected ? 'border-primary' : 'hover:border-border-strong'}"
>
	<!-- Selection checkbox — a sibling of the thumbnail button (never nested,
	     which would be invalid). Visible on hover, or always in selection mode. -->
	{#if selectable}
		<button
			type="button"
			onclick={(e) => {
				e.stopPropagation();
				onToggleSelect?.();
			}}
			aria-pressed={selected}
			aria-label={selected ? "Deselect recast" : "Select recast"}
			class="absolute left-2.5 top-2.5 z-30 grid size-6 place-items-center rounded-full border transition-opacity duration-200 motion-reduce:transition-none
				{selected
					? 'border-primary bg-primary text-background'
					: 'border-border-strong bg-background text-transparent opacity-0 group-hover/card:opacity-100 focus-visible:opacity-100'}
				{selectionMode && !selected ? 'opacity-100' : ''}"
		>
			<Check class="size-3.5" />
		</button>
	{/if}

	<!-- Thumbnail (fixed height — robust across grid breakpoints). A real link so
	     open-in-new-tab works; selection-mode clicks toggle instead of navigate.
	     draggable=false hands dragging to the parent article's card DnD. -->
	<a
		href="/dashboard/recasts/{recast.id}"
		draggable="false"
		onclick={(e) => {
			if (selectionMode) {
				e.preventDefault();
				onToggleSelect?.();
			}
		}}
		aria-label={selectionMode ? `Toggle selection of ${recast.title}` : `Open ${recast.title}`}
		class="relative block h-44 w-full shrink-0 overflow-hidden border-b border-border-low bg-paper"
	>
		{#if showPoster}
			<img
				src={recast.posterUrl}
				alt=""
				loading="lazy"
				onerror={() => (posterFailed = true)}
				class="absolute inset-0 h-full w-full object-cover"
			/>
		{:else}
			<span class="absolute inset-0 grid place-items-center">
				<Film class="size-6 text-border-strong" />
			</span>
		{/if}

		<!-- Dimming an image to float a play control over it is the one place an
		     alpha scrim is the actual job. -->
		<span
			class="absolute inset-0 grid place-items-center bg-background/40 opacity-0 transition-opacity duration-300 group-hover/card:opacity-100 motion-reduce:transition-none"
		>
			<span
				class="grid size-12 place-items-center rounded-full bg-foreground text-background transition-transform duration-200 group-active/card:scale-95 motion-reduce:transition-none"
			>
				<Play class="size-5 translate-x-0.5 fill-current" />
			</span>
		</span>

		{#if isShared}
			<span
				class="absolute right-2.5 top-2.5 z-20 flex items-center gap-1 rounded-md border border-border-low bg-background/90 px-1.5 py-0.5 text-caption font-medium text-muted-foreground"
			>
				<Share2 class="size-2.5" />
				Shared
			</span>
		{/if}

		<div
			class="absolute inset-x-2.5 bottom-2.5 z-20 flex items-center justify-between gap-2 text-caption font-medium tabular-nums"
		>
			{#if showViews}
				<span
					class="flex items-center gap-1 rounded-md border border-border-low bg-background/90 px-1.5 py-0.5 text-foreground"
				>
					<Eye class="size-3" />
					{formatCount(recast.views)}
				</span>
			{:else}
				<span></span>
			{/if}
			<span
				class="flex items-center gap-1 rounded-md border border-border-low bg-background/90 px-1.5 py-0.5 text-foreground"
			>
				{formatDuration(recast.durationSec)}
			</span>
		</div>
	</a>

	<!-- Meta -->
	<div class="flex flex-1 flex-col p-4">
		<div class="flex items-start gap-2">
			<div class="min-w-0 flex-1">
				<h3
					class="truncate font-display text-body-sm font-medium text-foreground"
					title={recast.title}
				>
					{recast.title}
				</h3>
				<p class="mt-1 text-caption text-muted-foreground">
					{formatRelative(recast.createdAt)} · {formatBytes(recast.sizeBytes)}
				</p>
			</div>

			<DropdownMenu.Root>
				<DropdownMenu.Trigger
					class="grid size-7 shrink-0 place-items-center rounded-md text-muted-foreground outline-none transition-colors hover:bg-paper hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50 motion-reduce:transition-none"
					aria-label="Recast options"
				>
					<MoreHorizontal class="size-4" />
				</DropdownMenu.Trigger>
				<DropdownMenu.Content align="end" sideOffset={6} class="w-52">
					<DropdownMenu.Item onclick={() => goto(`/dashboard/recasts/${recast.id}`)}>
						<Play class="size-4 text-muted-foreground" />
						Open
					</DropdownMenu.Item>
					<DropdownMenu.Item onclick={onrename}>
						<Pencil class="size-4 text-muted-foreground" />
						Rename
					</DropdownMenu.Item>
					<DropdownMenu.Item onclick={oncopylink}>
						<Link2 class="size-4 text-muted-foreground" />
						Copy link
					</DropdownMenu.Item>
					<DropdownMenu.Item onclick={() => goto(`/dashboard/recasts/${recast.id}/analytics`)}>
						<BarChart3 class="size-4 text-muted-foreground" />
						View analytics
					</DropdownMenu.Item>
					{#if onchangeposter}
						<DropdownMenu.Item onclick={onchangeposter}>
							<ImagePlus class="size-4 text-muted-foreground" />
							Change poster
						</DropdownMenu.Item>
					{/if}

					<!-- Move to folder -->
					<DropdownMenu.Sub>
						<DropdownMenu.SubTrigger>
							<FolderInput class="size-4 text-muted-foreground" />
							Move to
						</DropdownMenu.SubTrigger>
						<DropdownMenu.SubContent class="max-h-72 w-56 overflow-y-auto">
							<DropdownMenu.Item onclick={() => onmove(null)}>
								<Inbox class="size-4 text-muted-foreground" />
								<span class="flex-1">No folder</span>
								{#if !recast.folderId}<Check class="size-3.5 text-primary" />{/if}
							</DropdownMenu.Item>
							{#if sortedFolders.length > 0}
								<DropdownMenu.Separator />
								{#each sortedFolders as f (f.id)}
									<DropdownMenu.Item onclick={() => onmove(f.id)}>
										<span style="width: {folderDepth(f.path) * 10}px" class="shrink-0"></span>
										{#if f.color}
											<span class="size-2.5 shrink-0 rounded-xs" style="background:{f.color}"></span>
										{/if}
										<span class="flex-1 truncate">{f.name}</span>
										{#if recast.folderId === f.id}<Check class="size-3.5 text-primary" />{/if}
									</DropdownMenu.Item>
								{/each}
							{/if}
						</DropdownMenu.SubContent>
					</DropdownMenu.Sub>

					<!-- Tags -->
					<DropdownMenu.Sub>
						<DropdownMenu.SubTrigger>
							<TagIcon class="size-4 text-muted-foreground" />
							Tags
						</DropdownMenu.SubTrigger>
						<DropdownMenu.SubContent class="max-h-72 w-56 overflow-y-auto">
							{#if tags.length === 0}
								<div class="px-2 py-2 text-caption text-muted-foreground">
									No tags yet. Create one from the filter bar.
								</div>
							{:else}
								{#each tags as t (t.id)}
									<DropdownMenu.CheckboxItem
										checked={assignedSet.has(t.id)}
										onclick={() => ontoggletag(t.id)}
										closeOnSelect={false}
									>
										{#if t.color}
											<span class="size-2.5 shrink-0 rounded-full" style="background:{t.color}"></span>
										{/if}
										{t.name}
									</DropdownMenu.CheckboxItem>
								{/each}
							{/if}
						</DropdownMenu.SubContent>
					</DropdownMenu.Sub>

					<DropdownMenu.Separator />
					{#if onarchive}
						<DropdownMenu.Item onclick={onarchive}>
							<Archive class="size-4 text-muted-foreground" />
							Archive
						</DropdownMenu.Item>
					{/if}
					<DropdownMenu.Item
						onclick={ondelete}
						class="text-destructive/90 data-highlighted:text-destructive"
					>
						<Trash2 class="size-4" />
						Delete
					</DropdownMenu.Item>
				</DropdownMenu.Content>
			</DropdownMenu.Root>
		</div>

		<!-- Assigned tags -->
		{#if assignedTags.length > 0}
			<div class="mt-2.5 flex flex-wrap gap-1.5">
				{#each assignedTags as t (t.id)}
					<Chip label={t.name} color={t.color} class="py-0.5 text-caption" />
				{/each}
			</div>
		{/if}
	</div>
</article>
