<script lang="ts">
import { Grid2X2, List, Plus, Search, Settings2, X } from "@recast/icons";
import { Chip } from "@recast/ui/chip";
import * as Select from "@recast/ui/select";
import { cn } from "@recast/ui/utils";
import { focusOnMount } from "$lib/dashboard/focus";
import { tagsStore } from "$lib/dashboard/library.svelte";
import { isEditableTarget } from "$lib/dom/is-editable";

// Library search/sort/tag/view toolbar. Bindable filter state lives here so the
// page stays orchestration-only; folder selection is the breadcrumb's job, so
// the page passes the combined `filtersActive` + an `onclear` that also resets
// the folder.
let {
	query = $bindable(""),
	sortKey = $bindable("recent"),
	selectedTagIds = $bindable([]),
	viewMode = $bindable("grid"),
	total,
	shown,
	filtersActive,
	onclear,
	onmanagetags,
	oncreatetag,
}: {
	query?: string;
	sortKey?: string;
	selectedTagIds?: string[];
	viewMode?: "grid" | "list";
	total: number;
	shown: number;
	filtersActive: boolean;
	onclear: () => void;
	onmanagetags: () => void;
	oncreatetag: (name: string) => void;
} = $props();

let searchInput = $state<HTMLInputElement | null>(null);
let creatingTag = $state(false);
let newTagName = $state("");

const sorts = [
	{ label: "Newest first", value: "recent" },
	{ label: "Oldest first", value: "oldest" },
	{ label: "Name (A-Z)", value: "name" },
	{ label: "Largest first", value: "largest" },
];
const sortLabel = $derived(sorts.find((s) => s.value === sortKey)?.label ?? "Sort");

const views = [
	{ id: "grid" as const, label: "Grid", icon: Grid2X2 },
	{ id: "list" as const, label: "List", icon: List },
];

function toggleTag(id: string) {
	selectedTagIds = selectedTagIds.includes(id)
		? selectedTagIds.filter((t) => t !== id)
		: [...selectedTagIds, id];
}

function submitTag() {
	const name = newTagName.trim();
	creatingTag = false;
	newTagName = "";
	if (name) oncreatetag(name);
}

// Keyboard-first: "/" focuses the search from anywhere on the page (unless
// you're already typing); Escape clears it while focused.
function onWindowKeydown(e: KeyboardEvent) {
	if (e.key === "/" && !isEditableTarget(e.target)) {
		e.preventDefault();
		searchInput?.focus();
		searchInput?.select();
	}
}
</script>

<svelte:window onkeydown={onWindowKeydown} />

<div class="flex flex-col gap-3">
	<!-- Search + sort + view -->
	<div class="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
		<div
			class="flex h-9 items-center gap-2 rounded-lg border border-border-low bg-paper px-3 lg:w-72"
		>
			<Search class="size-4 shrink-0 text-muted-foreground" />
			<input
				bind:this={searchInput}
				type="text"
				bind:value={query}
				placeholder="Search recasts…"
				onkeydown={(e) => {
					if (e.key === "Escape") {
						query = "";
						e.currentTarget.blur();
					}
				}}
				class="w-full bg-transparent text-body-sm text-foreground outline-none placeholder:text-muted-foreground"
			/>
			{#if query}
				<button
					type="button"
					onclick={() => (query = "")}
					aria-label="Clear search"
					class="grid size-5 shrink-0 place-items-center rounded text-muted-foreground transition-colors hover:text-foreground motion-reduce:transition-none"
				>
					<X class="size-3.5" />
				</button>
			{:else}
				<kbd
					class="hidden shrink-0 rounded border border-border-low bg-background px-1.5 text-caption text-muted-foreground lg:inline"
				>
					/
				</kbd>
			{/if}
		</div>

		<div class="flex items-center gap-2">
			<Select.Root type="single" bind:value={sortKey}>
				<Select.Trigger aria-label="Sort recasts" class="h-9 w-40 text-body-sm">
					{sortLabel}
				</Select.Trigger>
				<Select.Content class="p-1">
					{#each sorts as s (s.value)}
						<Select.Item value={s.value} label={s.label}>{s.label}</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>

			<div
				class="flex h-9 items-center gap-0.5 rounded-lg border border-border-low bg-paper p-0.5"
				role="radiogroup"
				aria-label="Layout"
			>
				{#each views as v (v.id)}
					{@const active = viewMode === v.id}
					{@const Icon = v.icon}
					<button
						type="button"
						role="radio"
						aria-checked={active}
						aria-label={v.label}
						onclick={() => (viewMode = v.id)}
						class={cn(
							"grid size-8 place-items-center rounded-md transition-colors duration-200 motion-reduce:transition-none",
							active
								? "bg-background text-foreground shadow-craft-sm"
								: "text-muted-foreground hover:text-foreground",
						)}
					>
						<Icon class="size-4" />
					</button>
				{/each}
			</div>
		</div>
	</div>

	<!-- Tags + result count -->
	<div class="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
		<div class="flex flex-wrap items-center gap-1.5">
			{#each tagsStore.sorted as t (t.id)}
				<Chip
					label={t.name}
					color={t.color}
					selected={selectedTagIds.includes(t.id)}
					onclick={() => toggleTag(t.id)}
				/>
			{/each}
			{#if creatingTag}
				<input
					bind:value={newTagName}
					onblur={submitTag}
					onkeydown={(e) => {
						if (e.key === "Enter") e.currentTarget.blur();
						if (e.key === "Escape") {
							creatingTag = false;
							newTagName = "";
						}
					}}
					placeholder="Tag name"
					class="h-7 w-28 rounded-full border border-primary bg-background px-2.5 text-caption outline-none placeholder:text-muted-foreground"
					use:focusOnMount
				/>
			{:else}
				<button
					type="button"
					onclick={() => (creatingTag = true)}
					class="inline-flex items-center gap-1 rounded-full border border-dashed border-border-low px-2.5 py-1 text-caption font-medium text-muted-foreground transition-colors hover:border-border-strong hover:text-foreground motion-reduce:transition-none"
				>
					<Plus class="size-3" /> New tag
				</button>
			{/if}
			{#if tagsStore.items.length > 0}
				<button
					type="button"
					onclick={onmanagetags}
					class="inline-flex items-center gap-1 rounded-full px-2 py-1 text-caption font-medium text-muted-foreground transition-colors hover:bg-paper hover:text-foreground motion-reduce:transition-none"
				>
					<Settings2 class="size-3" /> Manage
				</button>
			{/if}
		</div>

		<div class="flex shrink-0 items-center gap-2 text-caption text-muted-foreground">
			<span class="tabular-nums">
				{filtersActive ? `${shown} of ${total}` : `${total} recast${total === 1 ? "" : "s"}`}
			</span>
			{#if filtersActive}
				<button
					type="button"
					onclick={onclear}
					class="font-medium text-muted-foreground underline-offset-4 transition-colors hover:text-foreground hover:underline motion-reduce:transition-none"
				>
					Clear filters
				</button>
			{/if}
		</div>
	</div>
</div>
