<script lang="ts">
	import { Ellipsis } from "@lucide/svelte";
	import * as DropdownMenu from "@recast/ui/dropdown-menu";
	import { cn } from "@recast/ui/utils";
	import type { RecastAccessory, RecastAction, RecastListItem } from "./types";

	interface Props {
		item: RecastListItem;
		index: number;
		onActivate: () => void;
	}

	let { item, index, onActivate }: Props = $props();

	const hasThumb = $derived(Boolean(item.iconImage));
	const primaryActions = $derived<RecastAction[]>(
		item.actions?.filter((a) => a.variant !== "destructive") ?? [],
	);
	const destructiveActions = $derived<RecastAction[]>(
		item.actions?.filter((a) => a.variant === "destructive") ?? [],
	);

	function accessoryClass(a: RecastAccessory) {
		const variants = {
			default: "bg-muted/80 text-muted-foreground border-border/40",
			success: "bg-success/10 text-success border-success/20",
			warning: "bg-warning/10 text-warning border-warning/20",
			destructive:
				"bg-destructive/10 text-destructive border-destructive/20",
			info: "bg-info/10 text-info border-info/20",
		} as const;
		return variants[a.variant ?? "default"];
	}

	function onKeydown(e: KeyboardEvent) {
		if (e.key === "Enter" || e.key === " ") {
			e.preventDefault();
			onActivate();
		}
	}
</script>

<div
	data-recast-card
	data-card-index={index}
	tabindex="0"
	role="button"
	aria-label={item.title}
	onclick={onActivate}
	onkeydown={onKeydown}
	class={cn(
		"group/card relative flex h-full cursor-pointer flex-col overflow-hidden rounded-2xl bg-card/40 text-left outline-none",
		"ring-1 ring-inset ring-border/50 shadow-(--shadow-craft-inset)",
		"transition-all duration-200 ease-out",
		"hover:ring-border hover:bg-card/70",
		"focus-visible:ring-primary/50 focus-visible:shadow-(--shadow-craft-inset-strong)",
	)}
>
	<div
		class={cn(
			"relative aspect-16/10 w-full shrink-0 overflow-hidden",
			"bg-linear-to-br from-muted/40 to-muted/10",
		)}
	>
		{#if hasThumb}
			<img
				src={item.iconImage}
				alt=""
				draggable="false"
				class="absolute inset-0 size-full object-cover transition-transform duration-[400ms] ease-out group-hover/card:scale-[1.03]"
			/>
			<div
				class="absolute inset-0 bg-linear-to-t from-background/40 via-transparent to-transparent"
			></div>
		{:else if item.icon}
			{@const Icon = item.icon}
			<div class="absolute inset-0 flex items-center justify-center">
				<span
					class={cn(
						"flex size-12 items-center justify-center rounded-2xl bg-background/70 ring-1 ring-inset ring-border/50 text-foreground/50",
						"transition-all duration-300 group-hover/card:scale-[1.05] group-hover/card:text-foreground/80",
						item.iconClass,
					)}
				>
					<Icon size={22} />
				</span>
			</div>
		{/if}

		{#if item.accessories && item.accessories.length > 0}
			<div
				class="absolute top-2 left-2 flex max-w-[calc(100%-3.5rem)] flex-wrap items-center gap-1"
			>
				{#each item.accessories as accessory}
					{@const AccIcon = accessory.icon}
					<span
						class={cn(
							"inline-flex items-center gap-1 rounded-md border px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wide backdrop-blur-md",
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

		{#if item.actions && item.actions.length > 0}
			<DropdownMenu.Root>
				<DropdownMenu.Trigger
					aria-label={`Actions for ${item.title}`}
					onclick={(e) => e.stopPropagation()}
					class={cn(
						"absolute top-2 right-2 flex size-7 items-center justify-center rounded-lg",
						"border border-border/60 bg-background/80 text-foreground/60 backdrop-blur-md",
						"transition-all duration-200",
						"opacity-0 group-hover/card:opacity-100 group-focus-within/card:opacity-100 data-[state=open]:opacity-100",
						"hover:bg-background hover:text-foreground",
						"focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/40",
					)}
				>
					<Ellipsis size={14} />
				</DropdownMenu.Trigger>
				<DropdownMenu.Content
					align="end"
					sideOffset={6}
					class="min-w-56 rounded-xl p-1.5"
				>
					{#each primaryActions as action (action.id)}
						{@const Icon = action.icon}
						<DropdownMenu.Item
							onSelect={() => action.onAction()}
							class="gap-2.5 rounded-lg px-2 py-1.5 text-[12px] font-medium"
						>
							{#if Icon}
								<Icon size={14} class="text-muted-foreground" />
							{/if}
							<span class="flex-1 truncate">{action.label}</span>
							{#if action.shortcut}
								<DropdownMenu.Shortcut
									class="font-mono text-[10px] tracking-tight text-muted-foreground"
								>
									{action.shortcut}
								</DropdownMenu.Shortcut>
							{/if}
						</DropdownMenu.Item>
					{/each}
					{#if destructiveActions.length > 0 && primaryActions.length > 0}
						<DropdownMenu.Separator class="my-1" />
					{/if}
					{#each destructiveActions as action (action.id)}
						{@const Icon = action.icon}
						<DropdownMenu.Item
							variant="destructive"
							onSelect={() => action.onAction()}
							class="gap-2.5 rounded-lg px-2 py-1.5 text-[12px] font-medium"
						>
							{#if Icon}
								<Icon size={14} />
							{/if}
							<span class="flex-1 truncate">{action.label}</span>
							{#if action.shortcut}
								<DropdownMenu.Shortcut
									class="font-mono text-[10px] tracking-tight text-destructive/60"
								>
									{action.shortcut}
								</DropdownMenu.Shortcut>
							{/if}
						</DropdownMenu.Item>
					{/each}
				</DropdownMenu.Content>
			</DropdownMenu.Root>
		{/if}
	</div>

	<div class="flex min-w-0 flex-1 flex-col gap-0.5 px-3.5 py-3">
		<h3
			class="truncate text-[13px] font-semibold tracking-tight text-foreground/90 group-hover/card:text-foreground"
		>
			{item.title}
		</h3>
		{#if item.subtitle}
			<p class="truncate text-[11px] font-medium text-muted-foreground">
				{item.subtitle}
			</p>
		{/if}
	</div>
</div>
