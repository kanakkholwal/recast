<script lang="ts">
	import { formatBytes, formatDuration } from "$lib/dashboard/format";
	import {
		recastsStore,
		STORAGE_QUOTA_BYTES,
	} from "$lib/dashboard/store.svelte";
	import { Clock, HardDrive } from "@lucide/svelte";

	// 5 hours of monthly recording allowance (matches Free tier vibe).
	const MONTHLY_MINUTES_QUOTA = 300;

	const usedBytes = $derived(recastsStore.usedBytes);
	const storagePct = $derived(
		Math.min(100, Math.round((usedBytes / STORAGE_QUOTA_BYTES) * 100)),
	);

	const totalSec = $derived(
		recastsStore.items.reduce((s, r) => s + r.durationSec, 0),
	);
	const minutesUsed = $derived(Math.round(totalSec / 60));
	const minutesPct = $derived(
		Math.min(100, Math.round((minutesUsed / MONTHLY_MINUTES_QUOTA) * 100)),
	);
</script>

<section class="glass-card flex flex-col gap-4 rounded-xl p-5">
	<div class="flex items-center gap-2">
		<HardDrive class="size-4 text-primary" />
		<h2 class="text-sm font-semibold text-foreground">Account usage</h2>
	</div>

	<!-- Storage -->
	<div>
		<div class="flex items-center justify-between text-xs">
			<span class="font-medium text-foreground">Storage</span>
			<span class="font-mono text-[11px] text-muted-foreground">
				{formatBytes(usedBytes)} / {formatBytes(STORAGE_QUOTA_BYTES)}
			</span>
		</div>
		<div class="mt-2 h-1.5 overflow-hidden rounded-full bg-foreground/8">
			<div
				class="h-full rounded-full bg-linear-to-r from-primary/70 to-primary transition-[width] duration-700 ease-[cubic-bezier(0.625,0.05,0,1)]"
				style="width: {storagePct}%"
			></div>
		</div>
		<p class="mt-1.5 text-[11px] text-muted-foreground">
			{100 - storagePct}% free
		</p>
	</div>

	<!-- Minutes -->
	<div>
		<div class="flex items-center justify-between text-xs">
			<span class="font-medium text-foreground">
				<Clock class="-mt-0.5 mr-1 inline size-3 text-muted-foreground" />
				Recorded this cycle
			</span>
			<span class="font-mono text-[11px] text-muted-foreground">
				{formatDuration(totalSec)} / {MONTHLY_MINUTES_QUOTA / 60}h
			</span>
		</div>
		<div class="mt-2 h-1.5 overflow-hidden rounded-full bg-foreground/8">
			<div
				class="h-full rounded-full bg-linear-to-r from-tertiary/70 to-tertiary transition-[width] duration-700 ease-[cubic-bezier(0.625,0.05,0,1)]"
				style="width: {minutesPct}%"
			></div>
		</div>
		<p class="mt-1.5 text-[11px] text-muted-foreground">
			{Math.max(0, MONTHLY_MINUTES_QUOTA - minutesUsed)} min remaining
		</p>
	</div>
</section>
