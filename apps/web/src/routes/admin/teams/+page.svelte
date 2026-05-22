<script lang="ts">
	import { Badge } from "@recast/ui/badge";

	let { data } = $props();
</script>

<header class="mb-6">
	<h1 class="text-2xl font-semibold tracking-tight">Teams</h1>
	<p class="mt-1 text-sm text-muted-foreground">
		{data.teams.length} {data.teams.length === 1 ? "team" : "teams"} total. Plan
		changes here are the only way to upgrade — there's no self-serve checkout.
	</p>
</header>

<div class="glass-card overflow-hidden rounded-xl">
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
				{#each data.teams as t (t.id)}
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
							No teams yet.
						</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
</div>
