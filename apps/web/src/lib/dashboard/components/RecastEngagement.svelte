<script lang="ts">
	import type { RecastEngagement } from "$lib/dashboard/activity";
	import { formatDuration, formatRelative } from "$lib/dashboard/format";
	import { MessageSquare, Smile } from "@recast/icons";

	// Surfaces the comments + reactions the player collects but never showed the
	// owner — read-only here (moderation still lives on the share page).
	let { engagement }: { engagement: RecastEngagement } = $props();
</script>

<section class="glass-card flex flex-col rounded-xl p-5">
	<header class="flex items-center gap-2">
		<MessageSquare class="size-4 text-muted-foreground" />
		<h2 class="text-sm font-semibold text-foreground">Engagement</h2>
	</header>

	<!-- Reactions -->
	<div class="mt-4">
		<div class="flex items-center gap-1.5 text-[10px] font-semibold uppercase tracking-[0.14em] text-muted-foreground">
			<Smile class="size-3" /> Reactions
		</div>
		{#if engagement.reactions.length === 0}
			<p class="mt-2 text-xs text-muted-foreground">No reactions yet.</p>
		{:else}
			<div class="mt-2 flex flex-wrap gap-1.5">
				{#each engagement.reactions as r (r.emoji)}
					<span class="inline-flex items-center gap-1.5 rounded-full border border-border-low/60 bg-card/40 px-2.5 py-1 text-sm">
						<span>{r.emoji}</span>
						<span class="font-mono text-xs font-semibold tabular-nums text-muted-foreground">{r.count}</span>
					</span>
				{/each}
			</div>
		{/if}
	</div>

	<!-- Comments -->
	<div class="mt-5">
		<div class="flex items-center gap-1.5 text-[10px] font-semibold uppercase tracking-[0.14em] text-muted-foreground">
			<MessageSquare class="size-3" /> Comments ({engagement.commentCount})
		</div>
		{#if engagement.recentComments.length === 0}
			<p class="mt-2 text-xs text-muted-foreground">No comments yet.</p>
		{:else}
			<ul class="mt-2 divide-y divide-border-low/40">
				{#each engagement.recentComments as c (c.createdAt + c.authorName)}
					<li class="py-2.5">
						<div class="flex items-center justify-between gap-2">
							<span class="truncate text-xs font-semibold text-foreground">{c.authorName}</span>
							<span class="shrink-0 font-mono text-[10px] text-muted-foreground">@ {formatDuration(c.atSeconds)}</span>
						</div>
						<p class="mt-0.5 text-xs text-muted-foreground">{c.body}</p>
						<p class="mt-0.5 text-[10px] text-muted-foreground/70">{formatRelative(c.createdAt)}</p>
					</li>
				{/each}
			</ul>
		{/if}
	</div>
</section>
