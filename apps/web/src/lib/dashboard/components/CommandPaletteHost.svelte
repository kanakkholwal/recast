<script lang="ts">
	import { commandPalette } from "$lib/dashboard/command-palette.svelte";
	import {
		BarChart3,
		CornerDownLeft,
		Film,
		LayoutDashboard,
		Plug,
		Plus,
		Search,
		Settings,
		SlidersHorizontal,
		User,
		Users,
		Zap,
	} from "@lucide/svelte";
	import { Badge } from "@recast/ui/badge";
	import * as Dialog from "@recast/ui/dialog";
	import { Kbd, KbdGroup } from "@recast/ui/kbd";
	import { cn } from "@recast/ui/utils";
	import { Command as CommandPrimitive } from "bits-ui";

	// The single command dialog + the global ⌘K binding. Mounted once (in the
	// header); every trigger opens THIS. Styled to match the desktop palette —
	// tall input, roomy rows, group headings, and a keyboard-hint footer — while
	// bits-ui handles filtering + keyboard navigation.

	// Reserved for the "fast upload with tags" flow — stubbed UI only for now.
	let fastUpload = $state(false);

	interface Entry {
		title: string;
		href: string;
		icon: typeof LayoutDashboard;
		keywords?: string;
	}

	const groups: { heading: string; items: Entry[] }[] = [
		{
			heading: "Quick actions",
			items: [
				{ title: "New Recast", href: "/dashboard/recasts", icon: Plus, keywords: "upload record video create" },
			],
		},
		{
			heading: "Pages",
			items: [
				{ title: "Home", href: "/dashboard", icon: LayoutDashboard, keywords: "overview dashboard" },
				{ title: "Recasts", href: "/dashboard/recasts", icon: Film, keywords: "videos library recordings" },
				{ title: "Analytics", href: "/dashboard/analytics", icon: BarChart3, keywords: "stats engagement views" },
				{ title: "Team", href: "/dashboard/team", icon: Users, keywords: "members organization workspace" },
			],
		},
		{
			heading: "Settings",
			items: [
				{ title: "Settings", href: "/dashboard/settings", icon: Settings },
				{ title: "Profile", href: "/dashboard/settings/profile", icon: User, keywords: "account name email" },
				{ title: "Integrations", href: "/dashboard/settings/integrations", icon: Plug, keywords: "cloudinary storage connect" },
				{ title: "Preferences", href: "/dashboard/settings/preferences", icon: SlidersHorizontal, keywords: "defaults autoupload" },
			],
		},
	];

	function onKeydown(e: KeyboardEvent) {
		if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
			e.preventDefault();
			commandPalette.toggle();
		}
	}
</script>

<svelte:window onkeydown={onKeydown} />

<Dialog.Root bind:open={commandPalette.open}>
	<Dialog.Content
		showCloseButton={false}
		class="top-[10vh] max-w-[calc(100%-2rem)] translate-y-0 gap-0 overflow-hidden rounded-xl p-0 shadow-2xl sm:max-w-xl"
	>
		<Dialog.Header class="sr-only">
			<Dialog.Title>Search</Dialog.Title>
			<Dialog.Description>Jump to a page or run an action</Dialog.Description>
		</Dialog.Header>

		<CommandPrimitive.Root class="flex w-full flex-col overflow-hidden">
			<!-- Input row -->
			<div class="flex items-center gap-2.5 border-b border-border/60 px-4">
				<Search class="size-4 shrink-0 text-muted-foreground/70" />
				<CommandPrimitive.Input
					placeholder="Search pages and actions…"
					class="h-12 w-full bg-transparent text-sm text-foreground placeholder:text-muted-foreground/60 focus:outline-none"
				/>
				<Kbd class="hidden shrink-0 sm:inline-flex">Esc</Kbd>
			</div>

			<!-- Stubbed control: with this on, a dropped video would auto-upload and
			     auto-tag. Non-functional for now — layout only. -->
			<div class="flex items-center gap-2.5 border-b border-border/50 px-4 py-3">
				<span class="grid size-7 shrink-0 place-items-center rounded-md bg-primary/10 text-primary">
					<Zap class="size-3.5" />
				</span>
				<span class="flex min-w-0 flex-col leading-tight">
					<span class="truncate text-[12.5px] font-medium text-foreground">Fast upload with tags</span>
					<span class="truncate text-[11px] text-muted-foreground">Auto-upload dropped videos and tag them</span>
				</span>
				<button
					type="button"
					role="switch"
					aria-checked={fastUpload}
					aria-label="Toggle fast upload with tags"
					disabled
					title="Coming soon"
					onclick={() => (fastUpload = !fastUpload)}
					class={cn(
						"ml-auto inline-flex h-4 w-7 shrink-0 cursor-not-allowed items-center rounded-full p-0.5 opacity-60 transition-colors",
						fastUpload ? "bg-primary" : "bg-foreground/15",
					)}
				>
					<span
						class={cn(
							"size-3 rounded-full bg-background shadow-sm transition-transform",
							fastUpload ? "translate-x-3" : "translate-x-0",
						)}
					></span>
				</button>
				<Badge variant="outline" class="shrink-0 text-[10px]">Soon</Badge>
			</div>

			<!-- Results -->
			<CommandPrimitive.List
				class="max-h-[22rem] overflow-y-auto overflow-x-hidden p-2 no-scrollbar"
			>
				<CommandPrimitive.Empty class="py-10 text-center text-sm text-muted-foreground">
					No results found.
				</CommandPrimitive.Empty>

				{#each groups as group (group.heading)}
					<CommandPrimitive.Group class="mb-1">
						<CommandPrimitive.GroupHeading
							class="px-2 pb-1 pt-2 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground/70"
						>
							{group.heading}
						</CommandPrimitive.GroupHeading>
						<CommandPrimitive.GroupItems>
							{#each group.items as item (item.href)}
								{@const Icon = item.icon}
								<CommandPrimitive.LinkItem
									href={item.href}
									keywords={item.keywords ? item.keywords.split(" ") : undefined}
									onSelect={() => commandPalette.hide()}
									class="group/item flex w-full cursor-pointer items-center gap-3 rounded-lg px-2 py-2 text-sm text-foreground/90 outline-none transition-colors data-selected:bg-muted data-selected:text-foreground"
								>
									<span
										class="flex size-8 shrink-0 items-center justify-center rounded-md bg-foreground/5 text-muted-foreground transition-colors group-data-selected/item:bg-primary/10 group-data-selected/item:text-primary"
									>
										<Icon class="size-4" />
									</span>
									<span class="flex-1 truncate font-medium">{item.title}</span>
								</CommandPrimitive.LinkItem>
							{/each}
						</CommandPrimitive.GroupItems>
					</CommandPrimitive.Group>
				{/each}
			</CommandPrimitive.List>

			<!-- Keyboard-hint footer -->
			<div
				class="flex items-center justify-between gap-3 border-t border-border/60 bg-muted/30 px-3 py-2 text-[11px] text-muted-foreground"
			>
				<span class="flex items-center gap-1.5">
					<Kbd><CornerDownLeft class="size-3" /></Kbd>
					<span>Run</span>
				</span>
				<span class="flex items-center gap-3">
					<span class="hidden items-center gap-1.5 sm:flex">
						<KbdGroup>
							<Kbd>↑</Kbd>
							<Kbd>↓</Kbd>
						</KbdGroup>
						<span>Navigate</span>
					</span>
					<span class="font-medium">Recast</span>
				</span>
			</div>
		</CommandPrimitive.Root>
	</Dialog.Content>
</Dialog.Root>
