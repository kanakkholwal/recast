<script lang="ts">
	import { Skeleton } from "@recast/ui/skeleton";

	import InlineError from "$lib/components/InlineError.svelte";

	let { data } = $props();

	function timeAgo(d: Date | string): string {
		const ms = Date.now() - new Date(d).getTime();
		const min = Math.floor(ms / 60_000);
		if (min < 1) return "just now";
		if (min < 60) return `${min}m ago`;
		const hr = Math.floor(min / 60);
		if (hr < 24) return `${hr}h ago`;
		return `${Math.floor(hr / 24)}d ago`;
	}

	/** Metadata is free-form JSON — flatten to `key: value` pairs for a compact,
	 *  scannable render instead of a raw JSON blob. */
	function metaPairs(meta: unknown): Array<{ k: string; v: string }> {
		if (!meta || typeof meta !== "object") return [];
		return Object.entries(meta as Record<string, unknown>).map(([k, v]) => ({
			k,
			v: typeof v === "object" ? JSON.stringify(v) : String(v),
		}));
	}
</script>

<header class="mb-6">
	<h1 class="text-2xl font-semibold tracking-tight">Audit log</h1>
	<p class="mt-1 text-sm text-muted-foreground">
		Append-only record of every admin action.
		{#await data.rows}{:then rows}
			Showing latest {rows.length}.
		{:catch}
			<!-- value hidden; the section below surfaces the error + retry -->
		{/await}
	</p>
</header>

<!-- Desktop table / mobile cards — see Users page for the rationale. -->
<div class="hidden overflow-hidden rounded-xl glass-card lg:block">
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
				{#await data.rows}
					{#each Array(8) as _, i (i)}
						<tr>
							<td class="px-4 py-3"><Skeleton class="h-3 w-28" /></td>
							<td class="px-4 py-3"><Skeleton class="h-3 w-24" /></td>
							<td class="px-4 py-3"><Skeleton class="h-3 w-32" /></td>
							<td class="px-4 py-3"><Skeleton class="h-3 w-20" /></td>
							<td class="px-4 py-3"><Skeleton class="h-3 w-40" /></td>
						</tr>
					{/each}
				{:then rows}
					{#each rows as r (r.id)}
						{@const pairs = metaPairs(r.metadata)}
						<tr class="transition-colors hover:bg-foreground/2">
							<td class="whitespace-nowrap px-4 py-3 text-[11px] text-muted-foreground">
								<time datetime={new Date(r.createdAt).toISOString()} title={new Date(r.createdAt).toLocaleString()}>
									{timeAgo(r.createdAt)}
								</time>
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
							<td class="px-4 py-3">
								{#if pairs.length}
									<div class="flex flex-wrap gap-1">
										{#each pairs as p (p.k)}
											<span class="inline-flex max-w-56 items-center gap-1 rounded-md bg-foreground/5 px-1.5 py-0.5 font-mono text-[10px]">
												<span class="text-muted-foreground">{p.k}</span>
												<span class="truncate text-foreground/80" title={p.v}>{p.v}</span>
											</span>
										{/each}
									</div>
								{:else}
									<span class="text-muted-foreground">—</span>
								{/if}
							</td>
						</tr>
					{:else}
						<tr>
							<td colspan="5" class="px-4 py-10 text-center text-sm text-muted-foreground">
								No admin actions yet.
							</td>
						</tr>
					{/each}
				{:catch}
					<tr>
						<td colspan="5" class="px-4 py-6">
							<InlineError message="Couldn't load the audit log." />
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
			<div class="glass-card space-y-2 rounded-xl p-3">
				<Skeleton class="h-3.5 w-32" />
				<Skeleton class="h-3 w-40" />
			</div>
		{/each}
	{:then rows}
		{#each rows as r (r.id)}
			{@const pairs = metaPairs(r.metadata)}
			<div class="glass-card rounded-xl p-3">
				<div class="flex items-center justify-between gap-2">
					<span class="truncate font-mono text-[11px] font-semibold uppercase tracking-wider">{r.action}</span>
					<time datetime={new Date(r.createdAt).toISOString()} title={new Date(r.createdAt).toLocaleString()} class="shrink-0 text-[11px] text-muted-foreground">
						{timeAgo(r.createdAt)}
					</time>
				</div>
				<div class="mt-1.5 flex flex-wrap items-center gap-x-3 gap-y-0.5 text-[11px] text-muted-foreground">
					<span class="font-mono">{r.actorEmail ?? r.actorId.slice(0, 8) + "…"}</span>
					{#if r.targetUserId}
						<a href="/admin/users/{r.targetUserId}" class="font-mono hover:text-foreground">→ {r.targetUserId.slice(0, 8)}…</a>
					{/if}
				</div>
				{#if pairs.length}
					<div class="mt-2 flex flex-wrap gap-1">
						{#each pairs as p (p.k)}
							<span class="inline-flex max-w-full items-center gap-1 rounded-md bg-foreground/5 px-1.5 py-0.5 font-mono text-[10px]">
								<span class="text-muted-foreground">{p.k}</span>
								<span class="truncate text-foreground/80" title={p.v}>{p.v}</span>
							</span>
						{/each}
					</div>
				{/if}
			</div>
		{:else}
			<div class="glass-card col-span-full rounded-xl px-4 py-10 text-center text-sm text-muted-foreground">
				No admin actions yet.
			</div>
		{/each}
	{:catch}
		<div class="col-span-full">
			<InlineError message="Couldn't load the audit log." />
		</div>
	{/await}
</div>
