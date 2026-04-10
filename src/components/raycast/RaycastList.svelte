<script lang="ts">
	import * as Command from "$components/ui/command";
	import { cn } from "$lib/utils";
	import { onMount, type Snippet } from "svelte";
	import ActionPanel from "./ActionPanel.svelte";
	import TopProgress from "./TopProgress.svelte";
	import type { RaycastAccessory, RaycastListItem } from "./types";

	interface Props {
		items: RaycastListItem[];
		isLoading?: boolean;
		searchPlaceholder?: string;
		emptyTitle?: string;
		emptyHint?: string;
		title?: string;
		subtitle?: string;
		toolbar?: Snippet;
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

	const sections = $derived.by(() => {
		const grouped = new Map<string, RaycastListItem[]>();
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

	function runPrimary(item: RaycastListItem) {
		if (item.onSelect) {
			item.onSelect();
		} else if (item.actions && item.actions.length > 0) {
			item.actions[0].onAction();
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (actionPanelOpen) return;
		// ⌘K / Ctrl+K opens action panel for selected item
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
			"[data-raycast-list] [data-slot='command-input']"
		);
		input?.focus();
	});

	function accessoryClass(a: RaycastAccessory) {
		const variants = {
			default: "bg-muted text-muted-foreground border-border",
			success: "bg-success/10 text-success border-success/20",
			warning: "bg-warning/10 text-warning border-warning/20",
			destructive: "bg-destructive/10 text-destructive border-destructive/20",
			info: "bg-info/10 text-info border-info/20",
		} as const;
		return variants[a.variant ?? "default"];
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="relative flex h-full flex-col" data-raycast-list>
	<TopProgress active={isLoading} />

	{#if title || toolbar}
		<header
			class="flex items-center justify-between gap-3 border-b border-border px-4 py-2.5"
		>
			{#if title}
				<div class="min-w-0">
					<h2 class="truncate text-[13px] font-semibold tracking-tight text-foreground">
						{title}
					</h2>
					{#if subtitle}
						<p class="truncate text-[11px] text-muted-foreground">{subtitle}</p>
					{/if}
				</div>
			{/if}
			{#if toolbar}
				<div class="flex shrink-0 items-center gap-1">
					{@render toolbar()}
				</div>
			{/if}
		</header>
	{/if}

	<Command.Root
		class="flex-1 rounded-none border-0 bg-transparent p-0"
		bind:value={selectedValue}
	>
		<Command.Input placeholder={searchPlaceholder} bind:value={searchValue} />

		<Command.List class="max-h-none flex-1 px-1 py-1">
			<Command.Empty>
				<div class="flex flex-col items-center justify-center gap-1 py-12 text-center">
					<p class="text-sm font-medium text-foreground">{emptyTitle}</p>
					{#if emptyHint}
						<p class="text-xs text-muted-foreground">{emptyHint}</p>
					{/if}
				</div>
			</Command.Empty>

			{#each sections as section (section.heading)}
				<Command.Group heading={section.heading || undefined}>
					{#each section.items as item (item.id)}
						{@const Icon = item.icon}
						<Command.Item
							value={item.id}
							keywords={[item.title, ...(item.keywords ?? []), item.subtitle ?? ""]}
							onSelect={() => runPrimary(item)}
							class="group/raycast-item h-9 gap-3 rounded-md px-2"
						>
							{#if item.iconImage}
								<img
									src={item.iconImage}
									alt=""
									class="size-5 shrink-0 rounded object-cover"
									draggable="false"
								/>
							{:else if Icon}
								<span
									class={cn(
										"flex size-5 shrink-0 items-center justify-center rounded text-muted-foreground",
										item.iconClass
									)}
								>
									<Icon size={14} />
								</span>
							{/if}

							<div class="flex min-w-0 flex-1 items-baseline gap-2">
								<span class="truncate text-[13px] font-medium text-foreground">
									{item.title}
								</span>
								{#if item.subtitle}
									<span class="truncate text-[11px] text-muted-foreground">
										{item.subtitle}
									</span>
								{/if}
							</div>

							{#if item.accessories && item.accessories.length > 0}
								<div class="flex shrink-0 items-center gap-1.5">
									{#each item.accessories as accessory}
										{@const AccIcon = accessory.icon}
										<span
											class={cn(
												"inline-flex items-center gap-1 rounded border px-1.5 py-0.5 text-[10px] font-medium",
												accessoryClass(accessory)
											)}
											title={accessory.tooltip}
										>
											{#if AccIcon}
												<AccIcon size={11} />
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

		<div
			class="flex h-9 items-center justify-between gap-3 border-t border-border bg-muted/30 px-3 text-[11px] text-muted-foreground"
		>
			<div class="flex items-center gap-3">
				{#if selectedItem && activeActions.length > 0}
					<span class="flex items-center gap-1">
						<kbd
							class="rounded border border-border bg-background px-1.5 py-0.5 font-mono"
							>↵</kbd
						>
						<span>{activeActions[0].label}</span>
					</span>
				{/if}
				{#if activeActions.length > 1}
					<span class="flex items-center gap-1">
						<kbd
							class="rounded border border-border bg-background px-1.5 py-0.5 font-mono"
							>⌘K</kbd
						>
						<span>Actions</span>
					</span>
				{/if}
			</div>
			<div class="flex items-center gap-3">
				<span class="flex items-center gap-1">
					<kbd class="rounded border border-border bg-background px-1.5 py-0.5 font-mono"
						>↑</kbd
					>
					<kbd class="rounded border border-border bg-background px-1.5 py-0.5 font-mono"
						>↓</kbd
					>
					<span>Navigate</span>
				</span>
			</div>
		</div>
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
