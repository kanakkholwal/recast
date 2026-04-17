<script lang="ts">
	import { Command as CommandIcon, Sparkles, X } from "@lucide/svelte";
	import * as Command from "@recast/ui/command";
	import { cn } from "@recast/ui/utils";
	import { onMount, type Snippet } from "svelte";
	import ActionPanel from "./ActionPanel.svelte";
	import TopProgress from "./TopProgress.svelte";
	import type { RecastAccessory, RecastListItem } from "./types";

	/** localStorage key for the first-visit tip. Global across all RecastList pages. */
	const KBD_HINT_KEY = "recast-kbd-hint-dismissed";

	interface Props {
		items: RecastListItem[];
		isLoading?: boolean;
		searchPlaceholder?: string;
		emptyTitle?: string;
		emptyHint?: string;
		title?: string;
		subtitle?: string;
		toolbar?: Snippet;
		class?: string;
	}

	let {
		items,
		isLoading = false,
		searchPlaceholder = "Search...",
		emptyTitle = "Nothing here",
		emptyHint = "",
		title,
		subtitle,
		toolbar,
	}: Props = $props();

	let searchValue = $state("");
	let selectedValue = $state<string>("");
	let actionPanelOpen = $state(false);
	let showKbdHint = $state(false);

	/** True if any item in the list exposes contextual actions. */
	const hasAnyActions = $derived(
		items.some((i) => i.actions && i.actions.length > 0),
	);

	function dismissHint() {
		showKbdHint = false;
		try {
			localStorage.setItem(KBD_HINT_KEY, "true");
		} catch {
			/* ignore — private mode etc. */
		}
	}

	const sections = $derived.by(() => {
		const grouped = new Map<string, RecastListItem[]>();
		for (const item of items) {
			const key = item.section ?? "";
			if (!grouped.has(key)) grouped.set(key, []);
			grouped.get(key)!.push(item);
		}
		return Array.from(grouped.entries()).map(([heading, sectionItems]) => ({
			heading,
			items: sectionItems,
		}));
	});

	const selectedItem = $derived(items.find((i) => i.id === selectedValue));
	const activeActions = $derived(selectedItem?.actions ?? []);

	function runPrimary(item: RecastListItem) {
		if (item.onSelect) {
			item.onSelect();
		} else if (item.actions && item.actions.length > 0) {
			item.actions[0].onAction();
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (actionPanelOpen) return;
		// ⌘ + K / Ctrl+K opens action panel for selected item
		if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
			if (selectedItem && activeActions.length > 0) {
				e.preventDefault();
				e.stopPropagation();
				actionPanelOpen = true;
			}
			return;
		}
		// ⌘+Enter runs secondary action
		if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
			if (selectedItem && activeActions.length >= 2) {
				e.preventDefault();
				e.stopPropagation();
				activeActions[1].onAction();
			}
		}
	}

	onMount(() => {
		// Focus the search input on mount for keyboard-first flow
		const input = document.querySelector<HTMLInputElement>(
			"[data-recast-list] [data-slot='command-input']",
		);
		input?.focus();

		// First-visit hint: show a one-time banner teaching ⌘ + K. Dismissed forever on close.
		try {
			if (localStorage.getItem(KBD_HINT_KEY) !== "true") {
				showKbdHint = true;
			}
		} catch {
			/* ignore */
		}
	});

	function accessoryClass(a: RecastAccessory) {
		const variants = {
			default: "bg-muted text-muted-foreground border-border/40",
			success: "bg-success/10 text-success border-success/20",
			warning: "bg-warning/10 text-warning border-warning/20",
			destructive:
				"bg-destructive/10 text-destructive border-destructive/20",
			info: "bg-info/10 text-info border-info/20",
		} as const;
		return variants[a.variant ?? "default"];
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<div
	class="relative flex h-full flex-col bg-background/50 selection:bg-primary/20 selection:text-primary font-sans select-none focus-visible:ring-3 focus-visible:ring-primary/20"
	data-recast-list
>
	<TopProgress active={isLoading} />

	{#if title || toolbar}
		<header
			class="flex items-center justify-between gap-4 px-8 pt-8 pb-4 shrink-0"
		>
			{#if title}
				<div class="min-w-0 space-y-0.5">
					<h2
						class="truncate text-xl font-semibold tracking-tight text-foreground"
					>
						{title}
					</h2>
					{#if subtitle}
						<p
							class="truncate text-[11px] font-medium text-foreground/30 uppercase tracking-[0.15em]"
						>
							{subtitle}
						</p>
					{/if}
				</div>
			{/if}
			{#if toolbar}
				<div class="flex shrink-0 items-center gap-2">
					{@render toolbar()}
				</div>
			{/if}
		</header>
	{/if}

	<!-- Keyboard Hint Banner -->
	{#if showKbdHint && hasAnyActions}
		<div
			class="mx-8 mb-4 flex shrink-0 items-center gap-3 rounded-2xl border border-border-subtle bg-foreground/[0.02] px-4 py-2.5 transition-all duration-300"
		>
			<span
				class="flex size-6 shrink-0 items-center justify-center rounded-full bg-primary/10 text-primary shadow-sm"
			>
				<Sparkles size={11} />
			</span>
			<p
				class="flex-1 truncate text-[12px] font-medium text-foreground/60"
			>
				<span>Select a row and press</span>
				<kbd
					class="mx-1.5 rounded-md border border-border-subtle bg-background px-1.5 py-0.5 font-mono text-[10px] font-bold text-foreground/80 shadow-craft-sm"
					>⌘K</kbd
				>
				<span>to manage actions</span>
			</p>
			<button
				onclick={dismissHint}
				class="shrink-0 size-6 rounded-full flex items-center justify-center text-foreground/20 hover:text-foreground hover:bg-foreground/5 transition-all"
				aria-label="Dismiss tip"
			>
				<X size={12} />
			</button>
		</div>
	{/if}

	<Command.Root
		class="flex-1 rounded-none border-0 bg-transparent p-0 flex flex-col"
		bind:value={selectedValue}
	>
		<div class="px-8 mb-4">
			<div class="group/search relative">
				<Command.Input
					placeholder={searchPlaceholder}
					bind:value={searchValue}
					wrapperClass="bg-transparent h-12! px-3 focus-visible:border-ring focus-visible:ring-primary/50 focus-visible:ring-3"
					class="h-12 text-[13px] font-medium border-none bg-transparent rounded-xl px-4  transition-all"
				/>
			</div>
		</div>

		<Command.List class="max-h-none flex-1 px-5 py-2 scrollbar-transparent">
			<Command.Empty>
				<div
					class="flex flex-col items-center justify-center gap-3 py-24 text-center"
				>
					<div
						class="size-12 rounded-2xl bg-foreground/2 flex items-center justify-center text-foreground/10"
					>
						<CommandIcon size={24} />
					</div>
					<div class="space-y-1">
						<p class="text-[14px] font-semibold text-foreground/80">
							{emptyTitle}
						</p>
						{#if emptyHint}
							<p
								class="text-[12px] font-medium text-foreground/30"
							>
								{emptyHint}
							</p>
						{/if}
					</div>
				</div>
			</Command.Empty>

			{#each sections as section (section.heading)}
				<Command.Group
					heading={section.heading || undefined}
					class="mb-5 last:mb-0 px-3"
				>
					{#each section.items as item (item.id)}
						{@const Icon = item.icon}
						<Command.Item
							value={item.id}
							keywords={[
								item.title,
								...(item.keywords ?? []),
								item.subtitle ?? "",
							]}
							onSelect={() => runPrimary(item)}
							class="group/recast-item h-11 transition-all duration-200 gap-3.5 rounded-xl px-3 hover:bg-foreground/[0.02] data-[selected=true]:bg-foreground/[0.04] data-[selected=true]:shadow-craft-sm border border-transparent data-[selected=true]:border-border-subtle"
						>
							{#if Icon}
								<span
									class={cn(
										"flex size-7 shrink-0 items-center justify-center rounded-[10px] bg-background border border-border-subtle text-foreground/30 group-hover/recast-item:text-foreground/60 data-[selected=true]:text-primary data-[selected=true]:bg-primary/[0.02] transition-all duration-300",
										item.iconClass,
									)}
								>
									<Icon size={13} />
								</span>
							{:else if item.iconImage}
								<img
									src={item.iconImage}
									alt=""
									class="size-7 shrink-0 rounded-[10px] object-cover border border-border-subtle group-hover/recast-item:scale-[1.05] transition-transform duration-300"
									draggable="false"
								/>
							{/if}

							<div
								class="flex min-w-0 flex-1 items-baseline gap-2.5"
							>
								<span
									class="truncate text-[13px] font-semibold text-foreground/80 group-hover/recast-item:text-foreground"
								>
									{item.title}
								</span>
								{#if item.subtitle}
									<span
										class="truncate text-[11px] font-medium text-foreground/30"
									>
										{item.subtitle}
									</span>
								{/if}
							</div>

							{#if item.actions && item.actions.length > 0 && selectedValue === item.id}
								<span
									class="invisible-ui opacity-0 group-hover/recast-item:opacity-100 flex items-center gap-1.5 rounded-md border border-border-subtle bg-background px-1.5 py-0.5 font-mono text-[9px] font-bold text-foreground/40 transition-all duration-300 shadow-craft-sm"
									aria-hidden="true"
								>
									<CommandIcon size={9} />
									K
								</span>
							{/if}

							{#if item.accessories && item.accessories.length > 0}
								<div class="flex shrink-0 items-center gap-2">
									{#each item.accessories as accessory}
										{@const AccIcon = accessory.icon}
										<span
											class={cn(
												"inline-flex items-center gap-1.5 rounded-md border px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-tight",
												accessoryClass(accessory),
											)}
											title={accessory.tooltip}
										>
											{#if AccIcon}
												<AccIcon size={10} />
											{/if}
											{#if accessory.text}
												<span>{accessory.text}</span>
											{/if}
										</span>
									{/each}
								</div>
							{/if}
						</Command.Item>
					{/each}
				</Command.Group>
			{/each}
		</Command.List>

		<footer
			class="mx-8 mb-6 flex h-10 items-center justify-between gap-4 rounded-full border border-border-subtle bg-foreground/2 backdrop-blur-3xl px-6 text-[9px] font-bold uppercase tracking-[0.15em] text-foreground/30 shrink-0"
		>
			<div class="flex items-center gap-8">
				{#if selectedItem && activeActions.length > 0}
					<span
						class="flex items-center gap-2 text-foreground/50 transition-colors"
					>
						<kbd class="opacity-50 text-[10px]">↵</kbd>
						<span>{activeActions[0].label}</span>
					</span>
				{/if}
				{#if hasAnyActions}
					<span
						class="flex items-center gap-2 text-primary/60 transition-colors"
					>
						<kbd class="opacity-50 text-[10px]">⌘K</kbd>
						<span>Actions Library</span>
					</span>
				{/if}
			</div>
			<div class="flex items-center gap-3">
				<span class="flex items-center gap-2">
					<kbd class="opacity-40 text-[10px]">↑↓</kbd>
					<span>Navigation</span>
				</span>
			</div>
		</footer>
	</Command.Root>

	{#if selectedItem && activeActions.length > 0}
		<ActionPanel
			bind:open={actionPanelOpen}
			actions={activeActions}
			title={selectedItem.title}
			onOpenChange={(v) => (actionPanelOpen = v)}
		/>
	{/if}
</div>
