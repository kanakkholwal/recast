<script lang="ts">
import { Search, X } from "@recast/icons";
import { Badge } from "@recast/ui/badge";
import { Button } from "@recast/ui/button";
import { Input } from "@recast/ui/input";
import * as Select from "@recast/ui/select";
import { Skeleton } from "@recast/ui/skeleton";

import InlineError from "$lib/components/InlineError.svelte";

let { data } = $props();

// Client-side filtering — the list is capped at 200 rows and streamed whole,
// so filtering in the browser is instant (no round-trip, no debounce).
let q = $state("");
let planFilter = $state("all");
const PLAN_LABEL: Record<string, string> = {
	all: "All plans",
	free: "Free",
	pro: "Pro",
	enterprise: "Enterprise",
};
const hasFilters = $derived(q.trim() !== "" || planFilter !== "all");

function matches(t: { name: string; slug: string; plan: string }): boolean {
	const needle = q.trim().toLowerCase();
	if (needle && !`${t.name} ${t.slug}`.toLowerCase().includes(needle)) return false;
	if (planFilter !== "all" && t.plan !== planFilter) return false;
	return true;
}
function clearFilters() {
	q = "";
	planFilter = "all";
}
</script>

<header class="mb-6">
	<h1 class="text-2xl font-semibold tracking-tight">Teams</h1>
	<p class="mt-1 text-sm text-muted-foreground">
		{#await data.teams}{:then teams}
			{teams.length} {teams.length === 1 ? "team" : "teams"} total.
		{:catch}
			<!-- value hidden; the section below surfaces the error + retry -->
		{/await}
		Plan changes here are the only way to upgrade. There's no self-serve checkout.
	</p>
</header>

<div class="mb-4 flex flex-wrap items-center gap-2 rounded-lg border border-border/40 bg-card/30 p-2">
	<div class="relative min-w-56 flex-1">
		<Search class="pointer-events-none absolute left-3 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
		<Input type="search" placeholder="Search by name or slug…" bind:value={q} aria-label="Search teams" class="h-9 pl-9" />
	</div>
	<Select.Root type="single" bind:value={planFilter}>
		<Select.Trigger class="h-9 w-36" aria-label="Filter by plan">
			{PLAN_LABEL[planFilter]}
		</Select.Trigger>
		<Select.Content>
			<Select.Item value="all">All plans</Select.Item>
			<Select.Item value="free">Free</Select.Item>
			<Select.Item value="pro">Pro</Select.Item>
			<Select.Item value="enterprise">Enterprise</Select.Item>
		</Select.Content>
	</Select.Root>
	{#if hasFilters}
		<Button type="button" size="sm" variant="ghost" class="gap-1.5 text-muted-foreground" onclick={clearFilters}>
			<X class="size-3.5" /> Clear
		</Button>
	{/if}
</div>

<!-- Desktop table / mobile cards — see Users page for the rationale. -->
<div class="hidden overflow-hidden rounded-xl glass-card lg:block">
	<div class="overflow-x-auto">
		<table class="w-full min-w-160 text-left text-sm">
			<thead class="border-b border-border/40 bg-foreground/2 text-[11px] uppercase tracking-[0.12em] text-muted-foreground">
				<tr>
					<th class="px-4 py-2.5">Team</th>
					<th class="px-4 py-2.5">Plan</th>
					<th class="px-4 py-2.5">Members</th>
					<th class="px-4 py-2.5">Created</th>
					<th class="px-4 py-2.5 text-right">Actions</th>
				</tr>
			</thead>
			<tbody class="divide-y divide-border/30">
				{#await data.teams}
					{#each Array(6) as _, i (i)}
						<tr>
							<td class="px-4 py-3">
								<div class="space-y-1.5">
									<Skeleton class="h-3.5 w-28" />
									<Skeleton class="h-3 w-20" />
								</div>
							</td>
							<td class="px-4 py-3"><Skeleton class="h-5 w-14" /></td>
							<td class="px-4 py-3"><Skeleton class="h-3 w-8" /></td>
							<td class="px-4 py-3"><Skeleton class="h-3 w-20" /></td>
							<td class="px-4 py-3 text-right"><Skeleton class="ml-auto h-6 w-16" /></td>
						</tr>
					{/each}
				{:then teams}
					{@const shown = teams.filter(matches)}
					{#each shown as t (t.id)}
						<tr class="transition-colors hover:bg-foreground/2">
							<td class="px-4 py-3">
								<a href="/admin/teams/{t.id}" class="block hover:text-primary">
									<span class="block truncate font-medium">{t.name}</span>
									<span class="block truncate font-mono text-xs text-muted-foreground">{t.slug}</span>
								</a>
							</td>
							<td class="px-4 py-3">
								{#if t.plan === "free"}
									<Badge variant="outline">free</Badge>
								{:else if t.plan === "pro"}
									<Badge variant="secondary">pro</Badge>
								{:else}
									<Badge variant="secondary" class="bg-primary/15 text-primary">enterprise</Badge>
								{/if}
							</td>
							<td class="px-4 py-3 tabular-nums">{t.memberCount}</td>
							<td class="px-4 py-3 text-muted-foreground">
								{new Date(t.createdAt).toLocaleDateString()}
							</td>
							<td class="px-4 py-3 text-right">
								<a
									href="/admin/teams/{t.id}"
									class="inline-flex items-center gap-1.5 rounded-md border border-border/40 px-2.5 py-1 text-xs font-medium transition-colors hover:bg-foreground/5"
								>
									Manage
								</a>
							</td>
						</tr>
					{:else}
						<tr>
							<td colspan="5" class="px-4 py-10 text-center text-sm text-muted-foreground">
								{hasFilters ? "No teams match your filters." : "No teams yet."}
							</td>
						</tr>
					{/each}
				{:catch}
					<tr>
						<td colspan="5" class="px-4 py-6">
							<InlineError message="Couldn't load teams." />
						</td>
					</tr>
				{/await}
			</tbody>
		</table>
	</div>
</div>

<div class="grid grid-cols-1 gap-2 sm:grid-cols-2 lg:hidden">
	{#await data.teams}
		{#each Array(6) as _, i (i)}
			<div class="glass-card rounded-xl p-3">
				<div class="space-y-1.5">
					<Skeleton class="h-3.5 w-28" />
					<Skeleton class="h-3 w-20" />
				</div>
				<div class="mt-3 flex items-center justify-between">
					<Skeleton class="h-5 w-14" />
					<Skeleton class="h-3 w-16" />
				</div>
			</div>
		{/each}
	{:then teams}
		{@const shown = teams.filter(matches)}
		{#each shown as t (t.id)}
			<a href="/admin/teams/{t.id}" class="glass-card block rounded-xl p-3 transition-colors hover:bg-foreground/2">
				<div class="flex items-start justify-between gap-2">
					<div class="min-w-0">
						<span class="block truncate font-medium">{t.name}</span>
						<span class="block truncate font-mono text-xs text-muted-foreground">{t.slug}</span>
					</div>
					{#if t.plan === "free"}
						<Badge variant="outline">free</Badge>
					{:else if t.plan === "pro"}
						<Badge variant="secondary">pro</Badge>
					{:else}
						<Badge variant="secondary" class="bg-primary/15 text-primary">enterprise</Badge>
					{/if}
				</div>
				<div class="mt-2.5 flex items-center justify-between text-xs text-muted-foreground">
					<span>{t.memberCount} {t.memberCount === 1 ? "member" : "members"} · {new Date(t.createdAt).toLocaleDateString()}</span>
					<span class="font-medium text-foreground/70">Manage →</span>
				</div>
			</a>
		{:else}
			<div class="glass-card col-span-full rounded-xl px-4 py-10 text-center text-sm text-muted-foreground">
				{hasFilters ? "No teams match your filters." : "No teams yet."}
			</div>
		{/each}
	{:catch}
		<div class="col-span-full">
			<InlineError message="Couldn't load teams." />
		</div>
	{/await}
</div>
