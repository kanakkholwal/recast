<script lang="ts">
	import { Badge } from "@recast/ui/badge";

	let { data } = $props();

	const statusVariant: Record<string, "default" | "outline" | "destructive" | "secondary"> = {
		active: "secondary",
		trialing: "secondary",
		past_due: "destructive",
		canceled: "outline",
		unpaid: "destructive",
		incomplete: "outline",
	};
</script>

<header class="mb-6">
	<h1 class="text-2xl font-semibold tracking-tight">Subscriptions</h1>
	<p class="mt-1 text-sm text-muted-foreground">
		Polar is the source of truth — this view mirrors what's in our DB.
		Refund or modify subscriptions from your <a class="font-semibold text-foreground hover:text-primary" href="https://polar.sh/" target="_blank" rel="noreferrer">Polar dashboard</a>.
	</p>
</header>

<div class="glass-card overflow-hidden rounded-xl">
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
				{#each data.rows as r (r.sub.id)}
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
							No subscriptions yet.
						</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
</div>
