<script lang="ts">
	import { Badge } from "@recast/ui/badge";
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

	const counts = $derived(data.counts);
	const subs = $derived(data.subs);

	const metricCards = $derived([
		{ label: "Total users", value: counts.total, icon: Users, trend: null },
		{ label: "Active users", value: counts.active, icon: UserCheck, trend: null },
		{ label: "On waitlist", value: counts.pending, icon: Hourglass, trend: null },
		{ label: "Admins", value: counts.admins, icon: Crown, trend: null },
		{ label: "Banned", value: counts.banned, icon: ShieldOff, trend: null },
		{ label: "Paid subscriptions", value: subs.active, icon: CreditCard, trend: null },
		{ label: "Signups (7d)", value: counts.signups7d, icon: TrendingUp, trend: null },
		{ label: "Signups (30d)", value: counts.signups30d, icon: Activity, trend: null },
		// Placeholder — recasts table doesn't exist yet (still localStorage on
		// the dashboard). Slot is reserved so the grid won't reflow when we
		// add `recasts` and start counting rows.
		{ label: "Videos", value: "—", icon: Film, trend: null, placeholder: true },
	]);

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
	{#each metricCards as card}
		{@const Icon = card.icon}
		<article
			class="glass-card flex flex-col gap-3 rounded-xl p-4"
			class:opacity-60={card.placeholder}
		>
			<div class="flex items-center justify-between">
				<span class="text-[11px] font-semibold uppercase tracking-[0.12em] text-muted-foreground">
					{card.label}
				</span>
				<Icon class="size-4 text-muted-foreground" />
			</div>
			<div class="text-2xl font-semibold tabular-nums tracking-tight">
				{card.value}
			</div>
		</article>
	{/each}
</section>

<div class="mt-10 grid gap-6 lg:grid-cols-5">
	<section class="glass-card rounded-xl p-5 lg:col-span-3">
		<div class="mb-4 flex items-center justify-between">
			<h2 class="text-sm font-semibold tracking-tight">Recent signups</h2>
			<a href="/admin/users" class="text-[11px] font-semibold uppercase tracking-[0.16em] text-muted-foreground hover:text-foreground">
				View all →
			</a>
		</div>
		<ul class="divide-y divide-border/40">
			{#each data.recentUsers as u (u.id)}
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
		<ul class="divide-y divide-border/40">
			{#each data.recentAudit as entry (entry.id)}
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
	</section>
</div>
