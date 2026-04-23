<script lang="ts">
	import PageShell from "$components/layout/PageShell.svelte";
	import { Search } from "@lucide/svelte";
	import { cn } from "@recast/ui/utils";
	import { onMount, tick, type Snippet } from "svelte";
	import RecastCard from "./RecastCard.svelte";
	import RecastRow from "./RecastRow.svelte";
	import TopProgress from "./TopProgress.svelte";
	import type { RecastAction, RecastListItem } from "./types";

	interface Props {
		items: RecastListItem[];
		isLoading?: boolean;
		searchPlaceholder?: string;
		emptyTitle?: string;
		emptyHint?: string;
		title: string;
		subtitle?: string;
		toolbar?: Snippet;
	}

	let {
		items,
		isLoading = false,
		searchPlaceholder = "Search…",
		emptyTitle = "Nothing here",
		emptyHint = "",
		title,
		subtitle,
		toolbar,
	}: Props = $props();

	let query = $state("");
	let searchEl = $state<HTMLInputElement | null>(null);
	let gridEl = $state<HTMLDivElement | null>(null);

	function matches(item: RecastListItem, q: string) {
		if (!q) return true;
		const haystack = [
			item.title,
			item.subtitle ?? "",
			...(item.keywords ?? []),
		]
			.join(" ")
			.toLowerCase();
		return haystack.includes(q);
	}

	const filtered = $derived.by(() => {
		const q = query.trim().toLowerCase();
		return items.filter((i) => matches(i, q));
	});

	const sections = $derived.by(() => {
		const grouped = new Map<string, RecastListItem[]>();
		for (const item of filtered) {
			const key = item.section ?? "";
			if (!grouped.has(key)) grouped.set(key, []);
			grouped.get(key)!.push(item);
		}
		return Array.from(grouped.entries()).map(([heading, sectionItems]) => ({
			heading,
			items: sectionItems,
		}));
	});

	const visibleOrder = $derived(filtered.map((i) => i.id));

	onMount(() => {
		tick().then(() => searchEl?.focus());
	});

	function focusCard(id: string) {
		const el = gridEl?.querySelector<HTMLElement>(
			`[data-card-id="${CSS.escape(id)}"]`,
		);
		el?.focus();
	}

	function focusFirstCard() {
		if (visibleOrder.length > 0) focusCard(visibleOrder[0]);
	}

	function activate(item: RecastListItem) {
		if (item.onSelect) item.onSelect();
		else if (item.actions && item.actions.length > 0) item.actions[0].onAction();
	}

	function handleSearchKeydown(e: KeyboardEvent) {
		if (e.key === "ArrowDown") {
			e.preventDefault();
			focusFirstCard();
		}
		if (e.key === "Enter" && filtered.length === 1) {
			e.preventDefault();
			activate(filtered[0]);
		}
		if (e.key === "Escape" && query) {
			e.preventDefault();
			query = "";
		}
	}

	function normalizeShortcut(s: string) {
		return s.replace(/\s+/g, "").toLowerCase();
	}

	function chordFromEvent(e: KeyboardEvent): string {
		const parts: string[] = [];
		if (e.metaKey || e.ctrlKey) parts.push("⌘");
		if (e.shiftKey) parts.push("⇧");
		if (e.altKey) parts.push("⌥");
		const key = e.key;
		if (key === "Enter") parts.push("↵");
		else if (key === "Backspace") parts.push("⌫");
		else if (key === " ") parts.push("space");
		else parts.push(key.toUpperCase());
		return normalizeShortcut(parts.join(""));
	}

	function findActionByChord(
		actions: RecastAction[] | undefined,
		chord: string,
	): RecastAction | undefined {
		if (!actions) return undefined;
		return actions.find(
			(a) => a.shortcut && normalizeShortcut(a.shortcut) === chord,
		);
	}

	function currentFocusedItem(): RecastListItem | undefined {
		const active = document.activeElement as HTMLElement | null;
		const id = active?.closest<HTMLElement>("[data-card-id]")?.dataset.cardId;
		if (!id) return undefined;
		return items.find((i) => i.id === id);
	}

	function moveFocus(delta: number) {
		const active = document.activeElement as HTMLElement | null;
		const id = active?.closest<HTMLElement>("[data-card-id]")?.dataset.cardId;
		if (!id) return;
		const idx = visibleOrder.indexOf(id);
		if (idx === -1) return;
		const next = Math.max(0, Math.min(visibleOrder.length - 1, idx + delta));
		focusCard(visibleOrder[next]);
	}

	function columnsPerRow(): number {
		if (!gridEl) return 1;
		const style = window.getComputedStyle(gridEl);
		const cols = style.gridTemplateColumns.split(" ").filter(Boolean);
		return Math.max(1, cols.length);
	}

	function handleGridKeydown(e: KeyboardEvent) {
		if (e.key === "ArrowRight") {
			e.preventDefault();
			moveFocus(1);
			return;
		}
		if (e.key === "ArrowLeft") {
			e.preventDefault();
			moveFocus(-1);
			return;
		}
		if (e.key === "ArrowDown") {
			e.preventDefault();
			moveFocus(columnsPerRow());
			return;
		}
		if (e.key === "ArrowUp") {
			e.preventDefault();
			const cols = columnsPerRow();
			const active = document.activeElement as HTMLElement | null;
			const id = active?.closest<HTMLElement>("[data-card-id]")?.dataset
				.cardId;
			const idx = id ? visibleOrder.indexOf(id) : -1;
			if (idx < cols) {
				searchEl?.focus();
				return;
			}
			moveFocus(-cols);
			return;
		}
		if ((e.metaKey || e.ctrlKey) && e.key.length === 1) {
			const item = currentFocusedItem();
			if (!item) return;
			const chord = chordFromEvent(e);
			const action = findActionByChord(item.actions, chord);
			if (action) {
				e.preventDefault();
				e.stopPropagation();
				action.onAction();
			}
		}
		if ((e.metaKey || e.ctrlKey) && e.key === "Backspace") {
			const item = currentFocusedItem();
			if (!item) return;
			const action = findActionByChord(item.actions, "⌘⌫");
			if (action) {
				e.preventDefault();
				e.stopPropagation();
				action.onAction();
			}
		}
	}
</script>

<PageShell {title} {subtitle} {toolbar}>
	<div class="relative h-full" data-recast-list>
		<TopProgress active={isLoading} />

		<div class="sticky top-0 z-10 bg-background/90 backdrop-blur-md">
			<div class="mx-8 my-4">
				<div class="relative">
					<Search
						size={14}
						class="pointer-events-none absolute top-1/2 left-3.5 -translate-y-1/2 text-muted-foreground"
					/>
					<input
						bind:this={searchEl}
						bind:value={query}
						onkeydown={handleSearchKeydown}
						placeholder={searchPlaceholder}
						type="text"
						spellcheck="false"
						class={cn(
							"h-10 w-full rounded-xl bg-card/40 pr-3 pl-10 text-[13px] font-medium text-foreground",
							"ring-1 ring-inset ring-border/50 shadow-(--shadow-craft-inset)",
							"placeholder:text-muted-foreground/70 outline-none",
							"transition-colors focus-visible:ring-primary/50 focus-visible:shadow-(--shadow-craft-inset-strong)",
						)}
					/>
					{#if query}
						<button
							type="button"
							onclick={() => {
								query = "";
								searchEl?.focus();
							}}
							class="absolute top-1/2 right-2 -translate-y-1/2 rounded-md px-2 py-0.5 font-mono text-[10px] font-semibold text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
						>
							esc
						</button>
					{/if}
				</div>
			</div>
		</div>

		{#if filtered.length === 0}
			<div class="flex flex-col items-center justify-center gap-3 px-8 py-24 text-center">
				<div
					class="flex size-14 items-center justify-center rounded-2xl bg-card/40 ring-1 ring-inset ring-border/40 text-muted-foreground"
				>
					<Search size={22} strokeWidth={1.6} />
				</div>
				<div class="space-y-1">
					<p class="text-[14px] font-semibold text-foreground/85">
						{query ? "No matches" : emptyTitle}
					</p>
					{#if query}
						<p class="text-[12px] font-medium text-muted-foreground">
							Try a different search term.
						</p>
					{:else if emptyHint}
						<p class="text-[12px] font-medium text-muted-foreground">
							{emptyHint}
						</p>
					{/if}
				</div>
			</div>
		{:else}
			<div
				bind:this={gridEl}
				onkeydown={handleGridKeydown}
				role="grid"
				tabindex="-1"
				class="px-8 pb-8 outline-none"
			>
				{#each sections as section, sIdx (section.heading || sIdx)}
					{@const layout = section.items[0]?.layout ?? "card"}
					<section class={cn(sIdx > 0 && "mt-8")}>
						{#if section.heading}
							<h2
								class="mb-3 text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground"
							>
								{section.heading}
							</h2>
						{/if}
						{#if layout === "row"}
							<div class="flex flex-col gap-1.5">
								{#each section.items as item (item.id)}
									<div data-card-id={item.id} class="min-w-0">
										<RecastRow
											{item}
											onActivate={() => activate(item)}
										/>
									</div>
								{/each}
							</div>
						{:else}
							<div
								class="grid grid-cols-[repeat(auto-fill,minmax(240px,1fr))] gap-3"
							>
								{#each section.items as item, i (item.id)}
									<div data-card-id={item.id} class="min-w-0">
										<RecastCard
											{item}
											index={i}
											onActivate={() => activate(item)}
										/>
									</div>
								{/each}
							</div>
						{/if}
					</section>
				{/each}
			</div>
		{/if}
	</div>
</PageShell>
