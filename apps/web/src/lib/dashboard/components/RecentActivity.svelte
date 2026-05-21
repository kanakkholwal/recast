<script lang="ts">
	import type { Activity } from "$lib/dashboard/activity";
	import { formatRelative } from "$lib/dashboard/format";
	import { Activity as ActivityIcon, CheckCircle2, Eye, Share2 } from "@lucide/svelte";

	let {
		activity,
		limit = 6,
	}: {
		activity: Activity[];
		limit?: number;
	} = $props();

	const items = $derived(activity.slice(0, limit));

	const kindMeta: Record<
		Activity["kind"],
		{ icon: typeof Eye; verb: string; tone: string }
	> = {
		viewed: { icon: Eye, verb: "watched", tone: "text-muted-foreground" },
		completed: { icon: CheckCircle2, verb: "finished", tone: "text-success" },
		shared: { icon: Share2, verb: "shared", tone: "text-primary" },
		downloaded: { icon: ActivityIcon, verb: "downloaded", tone: "text-muted-foreground" },
	};
</script>

<section class="glass-card flex h-full flex-col rounded-xl">
	<header class="flex items-center justify-between border-b border-border-low/50 px-5 py-3.5">
		<div class="flex items-center gap-2">
			<ActivityIcon class="size-4 text-primary" />
			<h2 class="text-sm font-semibold text-foreground">Recent activity</h2>
		</div>
		<a
			href="/dashboard/analytics"
			class="text-[11px] font-semibold uppercase tracking-[0.14em] text-muted-foreground transition-colors hover:text-foreground"
		>
			Analytics →
		</a>
	</header>

	{#if items.length === 0}
		<div class="flex flex-1 flex-col items-center justify-center px-5 py-10 text-center">
			<span class="glass-chip grid size-10 place-items-center rounded-lg text-muted-foreground">
				<ActivityIcon class="size-4" />
			</span>
			<p class="mt-3 text-xs text-muted-foreground">
				Once you share a recast, viewer activity lands here.
			</p>
		</div>
	{:else}
		<ul class="divide-y divide-border-low/40">
			{#each items as ev (ev.id)}
				{@const meta = kindMeta[ev.kind]}
				{@const Icon = meta.icon}
				<li class="flex items-start gap-3 px-5 py-3 transition-colors hover:bg-foreground/3">
					<span class="glass-chip mt-0.5 grid size-8 shrink-0 place-items-center rounded-lg {meta.tone}">
						<Icon class="size-3.5" />
					</span>
					<div class="min-w-0 flex-1">
						<p class="truncate text-sm text-foreground">
							<span class="font-semibold">{ev.viewer}</span>
							<span class="text-muted-foreground">{meta.verb}</span>
							<span class="font-medium">{ev.recastTitle}</span>
						</p>
						<p class="mt-0.5 text-[11px] text-muted-foreground">
							{formatRelative(ev.timestamp)}
							{#if ev.kind === "viewed"}
								· {ev.watchPct}% watched
							{/if}
						</p>
					</div>
				</li>
			{/each}
		</ul>
	{/if}
</section>
