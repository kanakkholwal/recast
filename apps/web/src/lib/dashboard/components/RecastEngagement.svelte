<script lang="ts">
import { MessageSquare, Smile } from "@recast/icons";
import type { RecastEngagement } from "$lib/dashboard/activity";
import { formatDuration, formatRelative } from "$lib/dashboard/format";

// Surfaces the comments + reactions the player collects but never showed the
// owner — read-only here (moderation still lives on the share page).
let { engagement }: { engagement: RecastEngagement } = $props();
</script>

<section class="surface flex h-full flex-col p-5">
	<header class="flex items-center gap-2">
		<MessageSquare class="size-4 text-muted-foreground" />
		<h2 class="font-display text-body font-medium text-foreground">Engagement</h2>
	</header>

	<!-- Reactions -->
	<div class="mt-4">
		<div class="flex items-center gap-1.5 text-caption font-medium text-muted-foreground">
			<Smile class="size-3" /> Reactions
		</div>
		{#if engagement.reactions.length === 0}
			<p class="mt-2 text-body-sm text-muted-foreground">No reactions yet.</p>
		{:else}
			<div class="mt-2 flex flex-wrap gap-1.5">
				{#each engagement.reactions as r (r.emoji)}
					<span class="inline-flex items-center gap-1.5 rounded-full border border-border-low bg-paper px-2.5 py-1 text-body-sm">
						<span>{r.emoji}</span>
						<span class="text-caption font-medium tabular-nums text-muted-foreground">{r.count}</span>
					</span>
				{/each}
			</div>
		{/if}
	</div>

	<!-- Comments -->
	<div class="mt-5">
		<div class="flex items-center gap-1.5 text-caption font-medium text-muted-foreground">
			<MessageSquare class="size-3" /> Comments ({engagement.commentCount})
		</div>
		{#if engagement.recentComments.length === 0}
			<p class="mt-2 text-body-sm text-muted-foreground">No comments yet.</p>
		{:else}
			<ul class="mt-2 divide-y divide-border-low">
				{#each engagement.recentComments as c (c.createdAt + c.authorName)}
					<li class="py-2.5">
						<div class="flex items-center justify-between gap-2">
							<span class="truncate text-body-sm font-medium text-foreground">{c.authorName}</span>
							<span class="shrink-0 text-caption tabular-nums text-muted-foreground">{formatDuration(c.atSeconds)}</span>
						</div>
						<p class="mt-0.5 text-body-sm text-muted-foreground">{c.body}</p>
						<p class="mt-0.5 text-caption text-muted-foreground">{formatRelative(c.createdAt)}</p>
					</li>
				{/each}
			</ul>
		{/if}
	</div>
</section>
