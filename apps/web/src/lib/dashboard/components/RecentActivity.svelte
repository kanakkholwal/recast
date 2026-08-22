<script lang="ts">
import type { Activity } from "$lib/dashboard/activity";
import { formatRelative } from "$lib/dashboard/format";
import { kindMeta } from "./RecentActivity.logic";
import { Activity as ActivityIcon } from "@recast/icons";

let {
	activity,
	limit = 6,
	linkHref = "/dashboard/analytics",
	linkLabel = "Analytics →",
}: {
	activity: Activity[];
	limit?: number;
	/** Header link target; pass `null` to hide it (e.g. when already on
	 *  the analytics or per-recast page, where it would be circular). */
	linkHref?: string | null;
	linkLabel?: string;
} = $props();

const items = $derived(activity.slice(0, limit));
</script>

<section class="surface flex h-full flex-col">
	<header class="flex items-center justify-between gap-4 border-b border-border-low px-5 py-3.5">
		<div class="flex items-center gap-2">
			<ActivityIcon class="size-4 text-muted-foreground" />
			<h2 class="font-display text-body font-medium text-foreground">Recent activity</h2>
		</div>
		{#if linkHref}
			<a
				href={linkHref}
				class="shrink-0 text-body-sm text-muted-foreground underline-offset-4 transition-colors hover:text-foreground hover:underline"
			>
				{linkLabel}
			</a>
		{/if}
	</header>

	{#if items.length === 0}
		<div class="flex flex-1 flex-col items-center justify-center px-5 py-10 text-center">
			<ActivityIcon class="size-6 text-border-strong" />
			<p class="mt-3 text-body-sm text-muted-foreground">
				Once you share a recast, viewer activity lands here.
			</p>
		</div>
	{:else}
		<ul class="divide-y divide-border-low">
			{#each items as ev (ev.id)}
				{@const meta = kindMeta[ev.kind]}
				{@const Icon = meta.icon}
				<li class="flex items-start gap-3 px-5 py-3 transition-colors hover:bg-paper motion-reduce:transition-none">
					<Icon class="mt-0.5 size-4 shrink-0 {meta.tone}" />
					<div class="min-w-0 flex-1">
						<p class="truncate text-body-sm text-foreground">
							<span class="font-medium">{ev.viewer}</span>
							<span class="text-muted-foreground">{meta.verb}</span>
							<span class="font-medium">{ev.recastTitle}</span>
						</p>
						<p class="mt-0.5 text-caption text-muted-foreground">
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
