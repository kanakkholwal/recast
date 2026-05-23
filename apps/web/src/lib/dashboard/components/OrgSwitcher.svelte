<script lang="ts">
	import { goto, invalidateAll } from "$app/navigation";
	import { authClient } from "$lib/auth/client";
	import CreateTeamDialog from "$lib/dashboard/components/CreateTeamDialog.svelte";
	import { Badge } from "@recast/ui/badge";
	import * as DropdownMenu from "@recast/ui/dropdown-menu";
	import { toast } from "@recast/ui/sonner";
	import { useSidebar } from "@recast/ui/sidebar";
	import { cn } from "@recast/ui/utils";
	import {
		Building2,
		Check,
		ChevronsUpDown,
		LoaderCircle,
		Plus,
		Sparkles,
	} from "@lucide/svelte";

	/**
	 * Team selector. Reads from /dashboard/+layout.server.ts so the list is
	 * always fresh server-side; switching calls Better Auth's
	 * `setActiveOrganization` then `invalidateAll()` so every loader re-runs
	 * against the new active org.
	 */

	type Membership = {
		organizationId: string;
		name: string;
		role: string;
		plan: string;
	};
	type Active = { id: string; name: string; plan: string; role: string };

	let {
		memberships,
		active,
	}: { memberships: Membership[]; active: Active } = $props();

	const sidebar = useSidebar();
	const open = $derived(sidebar.state === "expanded");

	let switching = $state<string | null>(null);
	let createOpen = $state(false);

	async function setActive(id: string) {
		if (switching || id === active.id) return;
		switching = id;
		const target = memberships.find((m) => m.organizationId === id);
		const targetName = target?.name ?? "team";
		try {
			await toast.promise(
				(async () => {
					const { error } = await authClient.organization.setActive({
						organizationId: id,
					});
					if (error) throw new Error(error.message ?? "Couldn't switch team.");
				})(),
				{
					loading: `Switching to ${targetName}…`,
					success: `Switched to ${targetName}.`,
					error: (err) => (err as Error)?.message ?? "Couldn't switch team.",
				},
			);
			await invalidateAll();
		} finally {
			// Always clear — a thrown rejection must not strand the row in
			// the "switching…" pseudo-loading state.
			switching = null;
		}
	}

	function initials(name: string) {
		return name
			.split(/\s+/)
			.filter(Boolean)
			.slice(0, 2)
			.map((w) => w[0]!.toUpperCase())
			.join("") || "T";
	}
</script>

<DropdownMenu.Root>
	<DropdownMenu.Trigger
		class={cn(
			"group/team flex w-full items-center gap-2.5 rounded-lg p-1.5 text-left outline-none transition-colors hover:bg-foreground/5 focus-visible:ring-2 focus-visible:ring-ring/50",
			!open && "justify-center",
		)}
		aria-label="Switch team"
	>
		<span
			class="grid size-8 shrink-0 place-items-center rounded-lg bg-foreground/8 text-[11px] font-bold text-foreground ring-1 ring-border/40"
		>
			{initials(active.name)}
		</span>
		{#if open}
			<span class="flex min-w-0 flex-1 flex-col leading-tight">
				<span class="truncate text-[12.5px] font-semibold text-foreground">
					{active.name}
				</span>
				<span class="flex items-center gap-1 text-[10px] uppercase tracking-wider text-muted-foreground">
					<span>{active.role}</span>
					<span aria-hidden="true">·</span>
					<span class={active.plan === "free" ? "" : "text-primary"}>
						{active.plan}
					</span>
				</span>
			</span>
			<ChevronsUpDown class="size-3.5 shrink-0 text-muted-foreground" />
		{/if}
	</DropdownMenu.Trigger>
	<DropdownMenu.Content align="start" sideOffset={6} class="w-64">
		<DropdownMenu.Label class="text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
			Your teams
		</DropdownMenu.Label>
		{#each memberships as m (m.organizationId)}
			<DropdownMenu.Item
				onclick={() => setActive(m.organizationId)}
				class={cn(
					"group/item flex cursor-pointer items-center justify-between gap-2.5",
					m.organizationId === active.id && "bg-foreground/5",
				)}
			>
				<span class="flex min-w-0 items-center gap-2.5">
					<span class="grid size-6 shrink-0 place-items-center rounded-md bg-foreground/8 text-[10px] font-bold text-foreground ring-1 ring-border/40">
						{initials(m.name)}
					</span>
					<span class="flex min-w-0 flex-col leading-tight">
						<span class="truncate text-sm font-medium text-foreground">
							{m.name}
						</span>
						<span class="truncate text-[10px] uppercase tracking-wider text-muted-foreground">
							{m.role} · {m.plan}
						</span>
					</span>
				</span>
				{#if switching === m.organizationId}
					<LoaderCircle class="size-3.5 shrink-0 animate-spin text-muted-foreground" />
				{:else if m.organizationId === active.id}
					<Check class="size-3.5 shrink-0 text-primary" />
				{/if}
			</DropdownMenu.Item>
		{/each}
		<DropdownMenu.Separator />
		<DropdownMenu.Item onclick={() => goto("/dashboard/team")}>
			<Building2 class="size-3.5 text-muted-foreground" />
			Manage current team
		</DropdownMenu.Item>
		<DropdownMenu.Item onSelect={() => (createOpen = true)}>
			<Plus class="size-3.5 text-muted-foreground" />
			Create a team
		</DropdownMenu.Item>
		{#if active.plan === "free"}
			<DropdownMenu.Separator />
			<DropdownMenu.Item onclick={() => goto("/pricing")}>
				<Sparkles class="size-3.5 text-primary" />
				<span class="text-foreground">Upgrade to Pro</span>
				<Badge variant="outline" class="ml-auto text-[10px]">50 seats</Badge>
			</DropdownMenu.Item>
		{/if}
	</DropdownMenu.Content>
</DropdownMenu.Root>

<CreateTeamDialog bind:open={createOpen} />
