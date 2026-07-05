<script lang="ts">
	import { focusOnMount } from "$lib/dashboard/focus";
	import { isEditableTarget } from "$lib/dom/is-editable";
	import { tagsStore } from "$lib/dashboard/library.svelte";
	import type { RecordingSource } from "$lib/dashboard/store.svelte";
	import { Chip } from "@recast/ui/chip";
	import * as Select from "@recast/ui/select";
	import { Plus, Search, Settings2, X } from "@lucide/svelte";

	// Library filter/search/sort toolbar. Bindable filter state lives here so the
	// page stays orchestration-only; folder selection is the FolderRail's job, so
	// the page passes the combined `filtersActive` + an `onclear` that also resets
	// the folder.
	let {
		query = $bindable(""),
		activeFilter = $bindable("all"),
		sortKey = $bindable("recent"),
		selectedTagIds = $bindable([]),
		total,
		shown,
		filtersActive,
		onclear,
		onmanagetags,
		oncreatetag,
	}: {
		query?: string;
		activeFilter?: RecordingSource | "all";
		sortKey?: string;
		selectedTagIds?: string[];
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

	const filters: { label: string; value: RecordingSource | "all" }[] = [
		{ label: "All", value: "all" },
		{ label: "Cloud", value: "cloud" },
		{ label: "Local", value: "local" },
	];
	const sorts = [
		{ label: "Newest first", value: "recent" },
		{ label: "Oldest first", value: "oldest" },
		{ label: "Name (A–Z)", value: "name" },
		{ label: "Largest first", value: "largest" },
	];
	const sortLabel = $derived(sorts.find((s) => s.value === sortKey)?.label ?? "Sort");

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
	<!-- Search + source filter + sort -->
	<div class="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
		<div class="flex items-center gap-2 rounded-lg border border-border-low/70 bg-card/50 px-3 py-2 backdrop-blur-sm lg:w-72">
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
				class="w-full bg-transparent text-sm text-foreground outline-none placeholder:text-muted-foreground/70"
			/>
			{#if query}
				<button type="button" onclick={() => (query = "")} aria-label="Clear search" class="grid size-5 place-items-center rounded text-muted-foreground transition-colors hover:text-foreground">
					<X class="size-3.5" />
				</button>
			{:else}
				<kbd class="hidden shrink-0 rounded border border-border-low/60 bg-background/60 px-1.5 font-mono text-[10px] text-muted-foreground/70 lg:inline">/</kbd>
			{/if}
		</div>

		<div class="flex items-center gap-2">
			<div class="flex items-center gap-1 rounded-lg border border-border-low/60 bg-card/40 p-1">
				{#each filters as f (f.value)}
					<button
						type="button"
						onclick={() => (activeFilter = f.value)}
						class="rounded-md px-3 py-1.5 text-xs font-semibold transition-colors duration-200
							{activeFilter === f.value ? 'bg-primary/12 text-foreground' : 'text-muted-foreground hover:text-foreground'}"
					>
						{f.label}
					</button>
				{/each}
			</div>

			<Select.Root type="single" bind:value={sortKey}>
				<Select.Trigger aria-label="Sort recasts" class="w-40 border-border-low/60 bg-card/40 text-xs font-semibold hover:border-border-low">
					{sortLabel}
				</Select.Trigger>
				<Select.Content class="p-1">
					{#each sorts as s (s.value)}
						<Select.Item value={s.value} label={s.label}>{s.label}</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
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
					class="h-7 w-28 rounded-full border border-primary/50 bg-background px-2.5 text-xs outline-none placeholder:text-muted-foreground/60"
					use:focusOnMount
				/>
			{:else}
				<button
					type="button"
					onclick={() => (creatingTag = true)}
					class="inline-flex items-center gap-1 rounded-full border border-dashed border-border-low/70 px-2.5 py-1 text-xs font-medium text-muted-foreground transition-colors hover:border-primary/50 hover:text-foreground"
				>
					<Plus class="size-3" /> New tag
				</button>
			{/if}
			{#if tagsStore.items.length > 0}
				<button
					type="button"
					onclick={onmanagetags}
					class="inline-flex items-center gap-1 rounded-full px-2 py-1 text-xs font-medium text-muted-foreground transition-colors hover:bg-foreground/8 hover:text-foreground"
				>
					<Settings2 class="size-3" /> Manage
				</button>
			{/if}
		</div>

		<div class="flex shrink-0 items-center gap-2 text-xs text-muted-foreground">
			<span class="font-mono tabular-nums">
				{filtersActive ? `${shown} of ${total}` : `${total} recast${total === 1 ? "" : "s"}`}
			</span>
			{#if filtersActive}
				<button type="button" onclick={onclear} class="font-medium text-muted-foreground transition-colors hover:text-foreground hover:underline">
					Clear filters
				</button>
			{/if}
		</div>
	</div>
</div>
