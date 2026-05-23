<script lang="ts">
	import { Badge } from "@recast/ui/badge";
	import { Skeleton } from "@recast/ui/skeleton";
	import {
		Activity,
		ClipboardList,
		CreditCard,
		Crown,
		Film,
		Hourglass,
		ShieldOff,
		TrendingUp,
		UserCheck,
		Users,
	} from "@lucide/svelte";

	let { data } = $props();

	const metricMeta = [
		{ key: "total", label: "Total users", icon: Users, source: "counts" },
		{ key: "active", label: "Active users", icon: UserCheck, source: "counts" },
		{ key: "pending", label: "On waitlist", icon: Hourglass, source: "counts" },
		{ key: "admins", label: "Admins", icon: Crown, source: "counts" },
		{ key: "banned", label: "Banned", icon: ShieldOff, source: "counts" },
		{ key: "active", label: "Paid subscriptions", icon: CreditCard, source: "subs" },
		{ key: "signups7d", label: "Signups (7d)", icon: TrendingUp, source: "counts" },
		{ key: "signups30d", label: "Signups (30d)", icon: Activity, source: "counts" },
	] as const;

	function timeAgo(d: Date | string): string {
		const ms = Date.now() - new Date(d).getTime();
		const min = Math.floor(ms / 60_000);
		if (min < 1) return "just now";
		if (min < 60) return `${min}m ago`;
		const hr = Math.floor(min / 60);
		if (hr < 24) return `${hr}h ago`;
		const d2 = Math.floor(hr / 24);
		return `${d2}d ago`;
	}
</script>

<header class="mb-8">
	<h1 class="text-2xl font-semibold tracking-tight">Overview</h1>
	<p class="mt-1 text-sm text-muted-foreground">
		High-level health of your user base, billing, and recent admin activity.
	</p>
</header>

<section class="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
	{#await data.metrics}
		{#each metricMeta as card (card.label)}
			{@const Icon = card.icon}
			<article class="glass-card flex flex-col gap-3 rounded-xl p-4">
				<div class="flex items-center justify-between">
					<span class="text-[11px] font-semibold uppercase tracking-[0.12em] text-muted-foreground">
						{card.label}
					</span>
					<Icon class="size-4 text-muted-foreground" />
				</div>
				<Skeleton class="h-7 w-16" />
			</article>
		{/each}
		<article class="glass-card flex flex-col gap-3 rounded-xl p-4 opacity-60">
			<div class="flex items-center justify-between">
				<span class="text-[11px] font-semibold uppercase tracking-[0.12em] text-muted-foreground">
					Videos
				</span>
				<Film class="size-4 text-muted-foreground" />
			</div>
			<div class="text-2xl font-semibold tabular-nums tracking-tight">—</div>
		</article>
	{:then m}
		{#each metricMeta as card (card.label)}
			{@const Icon = card.icon}
			{@const value =
				card.source === "subs"
					? (m.subs as Record<string, number>)[card.key]
					: (m.counts as Record<string, number>)[card.key]}
			<article class="glass-card flex flex-col gap-3 rounded-xl p-4">
				<div class="flex items-center justify-between">
					<span class="text-[11px] font-semibold uppercase tracking-[0.12em] text-muted-foreground">
						{card.label}
					</span>
					<Icon class="size-4 text-muted-foreground" />
				</div>
				<div class="text-2xl font-semibold tabular-nums tracking-tight">
					{value}
				</div>
			</article>
		{/each}
		<!-- Placeholder — recasts table doesn't exist yet (still localStorage on
			the dashboard). Slot is reserved so the grid won't reflow when we
			add `recasts` and start counting rows. -->
		<article class="glass-card flex flex-col gap-3 rounded-xl p-4 opacity-60">
			<div class="flex items-center justify-between">
				<span class="text-[11px] font-semibold uppercase tracking-[0.12em] text-muted-foreground">
					Videos
				</span>
				<Film class="size-4 text-muted-foreground" />
			</div>
			<div class="text-2xl font-semibold tabular-nums tracking-tight">—</div>
		</article>
	{/await}
</section>

<div class="mt-10 grid gap-6 lg:grid-cols-5">
	<section class="glass-card rounded-xl p-5 lg:col-span-3">
		<div class="mb-4 flex items-center justify-between">
			<h2 class="text-sm font-semibold tracking-tight">Recent signups</h2>
			<a href="/admin/users" class="text-[11px] font-semibold uppercase tracking-[0.16em] text-muted-foreground hover:text-foreground">
				View all →
			</a>
		</div>
		{#await data.recentUsers}
			<ul class="divide-y divide-border/40">
				{#each Array(5) as _, i (i)}
					<li class="flex items-center justify-between gap-3 py-2.5">
						<div class="min-w-0 flex-1 space-y-1.5">
							<Skeleton class="h-3.5 w-32" />
							<Skeleton class="h-3 w-44" />
						</div>
						<Skeleton class="h-3 w-14" />
					</li>
				{/each}
			</ul>
		{:then recentUsers}
			<ul class="divide-y divide-border/40">
				{#each recentUsers as u (u.id)}
					<li class="flex items-center justify-between gap-3 py-2.5">
						<div class="min-w-0">
							<a href="/admin/users/{u.id}" class="block truncate text-sm font-medium text-foreground hover:text-primary">
								{u.name}
							</a>
							<span class="block truncate text-xs text-muted-foreground">{u.email}</span>
						</div>
						<div class="flex shrink-0 items-center gap-2">
							{#if u.role === "admin"}
								<Badge variant="secondary" class="gap-1">
									<Crown class="size-3" /> admin
								</Badge>
							{/if}
							{#if u.status === "pending"}
								<Badge variant="outline" class="text-amber-600 dark:text-amber-400">
									waitlist
								</Badge>
							{/if}
							<span class="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
								{timeAgo(u.createdAt)}
							</span>
						</div>
					</li>
				{:else}
					<li class="py-3 text-sm text-muted-foreground">No users yet.</li>
				{/each}
			</ul>
		{/await}
	</section>

	<section class="glass-card rounded-xl p-5 lg:col-span-2">
		<div class="mb-4 flex items-center justify-between">
			<h2 class="flex items-center gap-2 text-sm font-semibold tracking-tight">
				<ClipboardList class="size-4 text-muted-foreground" />
				Recent admin actions
			</h2>
			<a href="/admin/audit" class="text-[11px] font-semibold uppercase tracking-[0.16em] text-muted-foreground hover:text-foreground">
				View all →
			</a>
		</div>
		{#await data.recentAudit}
			<ul class="divide-y divide-border/40">
				{#each Array(5) as _, i (i)}
					<li class="flex items-start justify-between gap-3 py-2.5">
						<div class="min-w-0 flex-1 space-y-1.5">
							<Skeleton class="h-3 w-28" />
							<Skeleton class="h-2.5 w-20" />
						</div>
						<Skeleton class="h-3 w-12" />
					</li>
				{/each}
			</ul>
		{:then recentAudit}
			<ul class="divide-y divide-border/40">
				{#each recentAudit as entry (entry.id)}
					<li class="flex items-start justify-between gap-3 py-2.5">
						<div class="min-w-0">
							<span class="block truncate font-mono text-[11px] font-semibold uppercase tracking-wider text-foreground/80">
								{entry.action}
							</span>
							{#if entry.targetUserId}
								<a href="/admin/users/{entry.targetUserId}" class="block truncate text-[11px] text-muted-foreground hover:text-foreground">
									target {entry.targetUserId.slice(0, 8)}…
								</a>
							{/if}
						</div>
						<span class="shrink-0 font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
							{timeAgo(entry.createdAt)}
						</span>
					</li>
				{:else}
					<li class="py-3 text-sm text-muted-foreground">No admin actions yet.</li>
				{/each}
			</ul>
		{/await}
	</section>
</div>
