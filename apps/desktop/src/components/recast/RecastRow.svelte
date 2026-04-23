<script lang="ts">
	import { ChevronRight, Ellipsis } from "@lucide/svelte";
	import * as DropdownMenu from "@recast/ui/dropdown-menu";
	import { cn } from "@recast/ui/utils";
	import type { RecastAccessory, RecastAction, RecastListItem } from "./types";

	interface Props {
		item: RecastListItem;
		onActivate: () => void;
	}

	let { item, onActivate }: Props = $props();

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
	tabindex="0"
	role="button"
	aria-label={item.title}
	onclick={onActivate}
	onkeydown={onKeydown}
	class={cn(
		"group/row relative flex h-14 cursor-pointer items-center gap-3 rounded-xl px-3 outline-none",
		"bg-card/40 ring-1 ring-inset ring-border/40 shadow-(--shadow-craft-inset)",
		"transition-all duration-150 ease-out",
		"hover:ring-border hover:bg-card/70",
		"focus-visible:ring-primary/50 focus-visible:shadow-(--shadow-craft-inset-strong)",
	)}
>
	{#if item.iconImage}
		<img
			src={item.iconImage}
			alt=""
			draggable="false"
			class="size-9 shrink-0 rounded-lg object-cover ring-1 ring-inset ring-border/40"
		/>
	{:else if item.icon}
		{@const Icon = item.icon}
		<span
			class={cn(
				"flex size-9 shrink-0 items-center justify-center rounded-lg",
				"bg-background/70 ring-1 ring-inset ring-border/40",
				"text-foreground/50 transition-colors group-hover/row:text-foreground/80",
				item.iconClass,
			)}
		>
			<Icon size={15} />
		</span>
	{/if}

	<div class="flex min-w-0 flex-1 flex-col gap-0.5">
		<h3 class="truncate text-[13px] font-semibold tracking-tight text-foreground/90 group-hover/row:text-foreground">
			{item.title}
		</h3>
		{#if item.subtitle}
			<p class="truncate text-[11px] font-medium text-muted-foreground">
				{item.subtitle}
			</p>
		{/if}
	</div>

	{#if item.accessories && item.accessories.length > 0}
		<div class="flex shrink-0 items-center gap-1.5">
			{#each item.accessories as accessory}
				{@const AccIcon = accessory.icon}
				<span
					class={cn(
						"inline-flex items-center gap-1 rounded-md border px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wide",
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
					"flex size-7 shrink-0 items-center justify-center rounded-lg",
					"border border-border/40 text-muted-foreground transition-all duration-150",
					"opacity-0 group-hover/row:opacity-100 group-focus-within/row:opacity-100 data-[state=open]:opacity-100",
					"hover:bg-background hover:text-foreground",
					"focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/40",
				)}
			>
				<Ellipsis size={13} />
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
	{:else}
		<ChevronRight
			size={14}
			class="shrink-0 text-muted-foreground/40 transition-transform duration-150 group-hover/row:translate-x-0.5 group-hover/row:text-muted-foreground"
		/>
	{/if}
</div>
