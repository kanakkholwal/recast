<script lang="ts">
	import { Search, X } from "@recast/icons";
	import { Badge } from "@recast/ui/badge";
	import { Button } from "@recast/ui/button";
	import { Input } from "@recast/ui/input";
	import * as Select from "@recast/ui/select";
	import { Skeleton } from "@recast/ui/skeleton";

	import InlineError from "$lib/components/InlineError.svelte";

	let { data } = $props();

	const statusVariant: Record<string, "default" | "outline" | "destructive" | "secondary"> = {
		active: "secondary",
		trialing: "secondary",
		past_due: "destructive",
		canceled: "outline",
		unpaid: "destructive",
		incomplete: "outline",
	};

	// Client-side filtering — streamed whole (≤200 rows), so it's instant.
	let q = $state("");
	let statusFilter = $state("all");
	const STATUS_LABEL: Record<string, string> = {
		all: "All statuses",
		active: "Active",
		trialing: "Trialing",
		past_due: "Past due",
		canceled: "Canceled",
		unpaid: "Unpaid",
		incomplete: "Incomplete",
	};
	const hasFilters = $derived(q.trim() !== "" || statusFilter !== "all");

	function matches(r: { sub: { status: string }; user: { name: string; email: string } }): boolean {
		const needle = q.trim().toLowerCase();
		if (needle && !`${r.user.name} ${r.user.email}`.toLowerCase().includes(needle)) return false;
		if (statusFilter !== "all" && r.sub.status !== statusFilter) return false;
		return true;
	}
	function clearFilters() {
		q = "";
		statusFilter = "all";
	}
</script>

<header class="mb-6">
	<h1 class="text-2xl font-semibold tracking-tight">Subscriptions</h1>
	<p class="mt-1 text-sm text-muted-foreground">
		Polar is the source of truth. This view mirrors our DB.
		Refund or modify subscriptions from your <a class="font-semibold text-foreground hover:text-primary" href="https://polar.sh/" target="_blank" rel="noreferrer">Polar dashboard</a>.
	</p>
</header>

<div class="mb-4 flex flex-wrap items-center gap-2 rounded-lg border border-border/40 bg-card/30 p-2">
	<div class="relative min-w-56 flex-1">
		<Search class="pointer-events-none absolute left-3 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
		<Input type="search" placeholder="Search by user name or email…" bind:value={q} aria-label="Search subscriptions" class="h-9 pl-9" />
	</div>
	<Select.Root type="single" bind:value={statusFilter}>
		<Select.Trigger class="h-9 w-40" aria-label="Filter by status">
			{STATUS_LABEL[statusFilter]}
		</Select.Trigger>
		<Select.Content>
			{#each Object.entries(STATUS_LABEL) as [value, label] (value)}
				<Select.Item {value}>{label}</Select.Item>
			{/each}
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
					<th class="px-4 py-2.5">User</th>
					<th class="px-4 py-2.5">Plan</th>
					<th class="px-4 py-2.5">Status</th>
					<th class="px-4 py-2.5">Renews</th>
					<th class="px-4 py-2.5">Polar ID</th>
				</tr>
			</thead>
			<tbody class="divide-y divide-border/30">
				{#await data.rows}
					{#each Array(6) as _, i (i)}
						<tr>
							<td class="px-4 py-3">
								<div class="space-y-1.5">
									<Skeleton class="h-3.5 w-28" />
									<Skeleton class="h-3 w-40" />
								</div>
							</td>
							<td class="px-4 py-3"><Skeleton class="h-3.5 w-12" /></td>
							<td class="px-4 py-3"><Skeleton class="h-5 w-16" /></td>
							<td class="px-4 py-3"><Skeleton class="h-3 w-20" /></td>
							<td class="px-4 py-3"><Skeleton class="h-3 w-28" /></td>
						</tr>
					{/each}
				{:then rows}
				{@const shown = rows.filter(matches)}
				{#each shown as r (r.sub.id)}
					<tr class="transition-colors hover:bg-foreground/2">
						<td class="px-4 py-3">
							<a href="/admin/users/{r.user.id}" class="block hover:text-primary">
								<span class="block truncate font-medium">{r.user.name}</span>
								<span class="block truncate text-xs text-muted-foreground">{r.user.email}</span>
							</a>
						</td>
						<td class="px-4 py-3 font-medium">{r.sub.plan}</td>
						<td class="px-4 py-3">
							<Badge variant={statusVariant[r.sub.status] ?? "outline"}>{r.sub.status}</Badge>
							{#if r.sub.cancelAtPeriodEnd}
								<Badge variant="outline" class="ml-1.5">cancels at period end</Badge>
							{/if}
						</td>
						<td class="px-4 py-3 text-muted-foreground">
							{r.sub.currentPeriodEnd ? new Date(r.sub.currentPeriodEnd).toLocaleDateString() : "—"}
						</td>
						<td class="px-4 py-3 font-mono text-[11px] text-muted-foreground">
							{r.sub.polarSubscriptionId?.slice(0, 16) ?? "—"}
						</td>
					</tr>
				{:else}
					<tr>
						<td colspan="5" class="px-4 py-10 text-center text-sm text-muted-foreground">
							{hasFilters ? "No subscriptions match your filters." : "No subscriptions yet."}
						</td>
					</tr>
				{/each}
				{:catch}
					<tr>
						<td colspan="5" class="px-4 py-6">
							<InlineError message="Couldn't load subscriptions." />
						</td>
					</tr>
				{/await}
			</tbody>
		</table>
	</div>
</div>

<div class="grid grid-cols-1 gap-2 sm:grid-cols-2 lg:hidden">
	{#await data.rows}
		{#each Array(6) as _, i (i)}
			<div class="glass-card rounded-xl p-3">
				<div class="space-y-1.5">
					<Skeleton class="h-3.5 w-28" />
					<Skeleton class="h-3 w-40" />
				</div>
				<div class="mt-3 flex items-center justify-between">
					<Skeleton class="h-5 w-16" />
					<Skeleton class="h-3 w-20" />
				</div>
			</div>
		{/each}
	{:then rows}
		{@const shown = rows.filter(matches)}
		{#each shown as r (r.sub.id)}
			<a href="/admin/users/{r.user.id}" class="glass-card block rounded-xl p-3 transition-colors hover:bg-foreground/2">
				<div class="flex items-start justify-between gap-2">
					<div class="min-w-0">
						<span class="block truncate font-medium">{r.user.name}</span>
						<span class="block truncate text-xs text-muted-foreground">{r.user.email}</span>
					</div>
					<div class="flex shrink-0 flex-wrap justify-end gap-1">
						<Badge variant={statusVariant[r.sub.status] ?? "outline"}>{r.sub.status}</Badge>
						{#if r.sub.cancelAtPeriodEnd}
							<Badge variant="outline">cancels at period end</Badge>
						{/if}
					</div>
				</div>
				<div class="mt-2.5 flex items-center justify-between text-xs text-muted-foreground">
					<span class="font-medium text-foreground/80">{r.sub.plan}</span>
					<span>Renews {r.sub.currentPeriodEnd ? new Date(r.sub.currentPeriodEnd).toLocaleDateString() : "—"}</span>
				</div>
			</a>
		{:else}
			<div class="glass-card col-span-full rounded-xl px-4 py-10 text-center text-sm text-muted-foreground">
				{hasFilters ? "No subscriptions match your filters." : "No subscriptions yet."}
			</div>
		{/each}
	{:catch}
		<div class="col-span-full">
			<InlineError message="Couldn't load subscriptions." />
		</div>
	{/await}
</div>
