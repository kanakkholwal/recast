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
			<!-- Icon-as-hero placeholder. Borrows the editor-tour rail's techy
			     framing (dot grid + primary glow + glass tile) so a thumbnail-
			     less recording reads as "ready for a frame" instead of empty. -->
			<div
				aria-hidden="true"
				class="absolute inset-0 opacity-60"
				style="background-image: radial-gradient(circle, color-mix(in srgb, var(--color-foreground) 8%, transparent) 1px, transparent 1px); background-size: 14px 14px;"
			></div>
			<div
				aria-hidden="true"
				class="pointer-events-none absolute -bottom-8 left-1/2 size-36 -translate-x-1/2 rounded-full opacity-70"
				style="background: radial-gradient(closest-side, color-mix(in srgb, var(--color-primary) 22%, transparent), transparent 75%);"
			></div>
			<div class="absolute inset-0 flex items-center justify-center">
				<span
					class={cn(
						"flex size-14 items-center justify-center rounded-xl bg-card/55 ring-1 ring-inset ring-border/60 text-foreground/70 backdrop-blur-sm shadow-craft-sm",
						"transition-all duration-300 group-hover/card:scale-[1.05] group-hover/card:text-foreground",
						item.iconClass,
					)}
				>
					<Icon size={22} class="drop-shadow-[0_4px_10px_color-mix(in_srgb,var(--color-primary)_30%,transparent)]" />
				</span>
			</div>
		{/if}

		<!-- CRT-style corner brackets. Always-on accent that ties recording
		     tiles to the marketing rail's visual language. Sized smaller than
		     the web card because the desktop tile is denser. -->
		<span aria-hidden="true" class="pointer-events-none absolute left-1.5 top-1.5 z-10 size-2 border-l border-t border-foreground/30"></span>
		<span aria-hidden="true" class="pointer-events-none absolute right-1.5 top-1.5 z-10 size-2 border-r border-t border-foreground/30"></span>
		<span aria-hidden="true" class="pointer-events-none absolute bottom-1.5 left-1.5 z-10 size-2 border-b border-l border-foreground/30"></span>
		<span aria-hidden="true" class="pointer-events-none absolute bottom-1.5 right-1.5 z-10 size-2 border-b border-r border-foreground/30"></span>

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
					class="min-w-48 rounded-xl p-1"
				>
					{#each primaryActions as action (action.id)}
						{@const Icon = action.icon}
						<DropdownMenu.Item
							onSelect={() => action.onAction()}
							class="gap-2.5 rounded-lg px-2 py-1.5 font-medium"
						>
							{#if Icon}
								<Icon size={14} class="text-muted-foreground size-3" />
							{/if}
							<span class="flex-1 truncate text-[11px]">{action.label}</span>
							{#if action.shortcut}
								<DropdownMenu.Shortcut
									class="font-mono text-[10px] tracking-wide text-muted-foreground"
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
							class="gap-2.5 rounded-lg px-2 py-1.5 text-[10px] font-medium"
						>
							{#if Icon}
								<Icon size={12} class="text-destructive/60 size-3" />
							{/if}
							<span class="flex-1 truncate text-[11px]">{action.label}</span>
							{#if action.shortcut}
								<DropdownMenu.Shortcut
									class="font-mono text-[10px] tracking-wide text-destructive/60"
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
