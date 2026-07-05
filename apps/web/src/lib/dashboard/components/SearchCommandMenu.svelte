<script lang="ts">
	import {
	  BarChart3,
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
	import * as Command from "@recast/ui/command";
	import { Kbd } from "@recast/ui/kbd";
	import { cn } from "@recast/ui/utils";

	// Owns its own open state + the global ⌘K shortcut. The trigger (header
	// search field) and the dialog live together so the header stays declarative.
	let open = $state(false);

	// Reserved for the "fast upload with tags" flow — stubbed UI only for now.
	let fastUpload = $state(false);

	interface Entry {
		title: string;
		href: string;
		icon: typeof LayoutDashboard;
		keywords?: string;
	}

	const quickActions: Entry[] = [
		{ title: "New Recast", href: "/dashboard/recasts", icon: Plus, keywords: "upload record video create" },
	];

	const pages: Entry[] = [
		{ title: "Home", href: "/dashboard", icon: LayoutDashboard, keywords: "overview dashboard" },
		{ title: "Recasts", href: "/dashboard/recasts", icon: Film, keywords: "videos library recordings" },
		{ title: "Analytics", href: "/dashboard/analytics", icon: BarChart3, keywords: "stats engagement views" },
		{ title: "Team", href: "/dashboard/team", icon: Users, keywords: "members organization workspace" },
	];

	const settingsPages: Entry[] = [
		{ title: "Settings", href: "/dashboard/settings", icon: Settings },
		{ title: "Profile", href: "/dashboard/settings/profile", icon: User, keywords: "account name email" },
		{ title: "Integrations", href: "/dashboard/settings/integrations", icon: Plug, keywords: "cloudinary storage connect" },
		{ title: "Preferences", href: "/dashboard/settings/preferences", icon: SlidersHorizontal, keywords: "defaults autoupload" },
	];

	function onKeydown(e: KeyboardEvent) {
		if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
			e.preventDefault();
			open = !open;
		}
	}
</script>

<svelte:window onkeydown={onKeydown} />

<button
	type="button"
	onclick={() => (open = true)}
	aria-label="Search pages and actions"
	title="Search (⌘K)"
	class="group flex h-9 w-full items-center gap-2.5 rounded-lg border border-input/40 bg-input/80 px-3 text-left text-sm text-muted-foreground transition-colors hover:border-input/70 hover:bg-input/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/50"
>
	<Search class="size-4 shrink-0 opacity-60 transition-opacity group-hover:opacity-80" />
	<span class="flex-1 truncate">Search pages and actions…</span>
	<Kbd class="hidden shrink-0 sm:inline-flex">
		<span class="text-[9px] font-semibold">⌘</span>
		<span class="text-[10px]">K</span>
	</Kbd>
</button>

<Command.Dialog
	bind:open
	title="Search"
	description="Jump to a page or run an action"
	class="max-w-xl"
>
	<Command.Input placeholder="Search pages and actions…" />

	<!-- Stubbed control: with this on, a dropped video would auto-upload and
	     auto-tag. Non-functional for now — layout only. -->
	<div
		class="flex items-center gap-2.5 border-b border-border/50 px-3 py-2.5"
	>
		<span class="grid size-6 shrink-0 place-items-center rounded-md bg-primary/10 text-primary">
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

	<Command.List>
		<Command.Empty>No results found.</Command.Empty>

		<Command.Group heading="Quick actions">
			{#each quickActions as item (item.href)}
				{@const Icon = item.icon}
				<Command.LinkItem
					href={item.href}
					keywords={item.keywords ? item.keywords.split(" ") : undefined}
					onSelect={() => (open = false)}
				>
					<Icon class="size-4 text-muted-foreground" />
					<span>{item.title}</span>
				</Command.LinkItem>
			{/each}
		</Command.Group>

		<Command.Group heading="Pages">
			{#each pages as item (item.href)}
				{@const Icon = item.icon}
				<Command.LinkItem
					href={item.href}
					keywords={item.keywords ? item.keywords.split(" ") : undefined}
					onSelect={() => (open = false)}
				>
					<Icon class="size-4 text-muted-foreground" />
					<span>{item.title}</span>
				</Command.LinkItem>
			{/each}
		</Command.Group>

		<Command.Group heading="Settings">
			{#each settingsPages as item (item.href)}
				{@const Icon = item.icon}
				<Command.LinkItem
					href={item.href}
					keywords={item.keywords ? item.keywords.split(" ") : undefined}
					onSelect={() => (open = false)}
				>
					<Icon class="size-4 text-muted-foreground" />
					<span>{item.title}</span>
				</Command.LinkItem>
			{/each}
		</Command.Group>
	</Command.List>
</Command.Dialog>
