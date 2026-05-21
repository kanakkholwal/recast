<script lang="ts">
	let { data } = $props();
</script>

<header class="mb-6">
	<h1 class="text-2xl font-semibold tracking-tight">Audit log</h1>
	<p class="mt-1 text-sm text-muted-foreground">
		Append-only record of every admin action. Showing latest {data.rows.length}.
	</p>
</header>

<div class="glass-card overflow-hidden rounded-xl">
	<div class="overflow-x-auto">
		<table class="w-full min-w-180 text-left text-sm">
			<thead class="border-b border-border/40 bg-foreground/2 text-[11px] uppercase tracking-[0.12em] text-muted-foreground">
				<tr>
					<th class="px-4 py-2.5">When</th>
					<th class="px-4 py-2.5">Action</th>
					<th class="px-4 py-2.5">Actor</th>
					<th class="px-4 py-2.5">Target</th>
					<th class="px-4 py-2.5">Metadata</th>
				</tr>
			</thead>
			<tbody class="divide-y divide-border/30">
				{#each data.rows as r (r.id)}
					<tr class="transition-colors hover:bg-foreground/2">
						<td class="px-4 py-3 font-mono text-[11px] text-muted-foreground">
							{new Date(r.createdAt).toLocaleString()}
						</td>
						<td class="px-4 py-3 font-mono text-[11px] font-semibold uppercase tracking-wider">
							{r.action}
						</td>
						<td class="px-4 py-3">
							<span class="block truncate font-mono text-[11px]">{r.actorEmail ?? r.actorId.slice(0, 8) + "…"}</span>
						</td>
						<td class="px-4 py-3">
							{#if r.targetUserId}
								<a href="/admin/users/{r.targetUserId}" class="font-mono text-[11px] text-muted-foreground hover:text-foreground">
									{r.targetUserId.slice(0, 8)}…
								</a>
							{:else}
								<span class="text-muted-foreground">—</span>
							{/if}
						</td>
						<td class="px-4 py-3 font-mono text-[10px] text-muted-foreground">
							{r.metadata ? JSON.stringify(r.metadata) : "—"}
						</td>
					</tr>
				{:else}
					<tr>
						<td colspan="5" class="px-4 py-10 text-center text-sm text-muted-foreground">
							No admin actions yet.
						</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
</div>
