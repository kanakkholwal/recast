<script lang="ts">
	import { formatBytes } from "$lib/dashboard/format";
	import { quotaStore } from "$lib/dashboard/store.svelte";
	import { type UsageTone, usageView } from "./UsageMeter.logic";
	import { HardDrive, Link2 } from "@recast/icons";

	// Reactive snapshot pulled from the layout-injected quota. When the
	// workspace plan is Enterprise (no cap) the bars render at 0% and the
	// limit row reads "Unlimited" — same component, no special path.
	const view = $derived(usageView(quotaStore.value));

	// Neutral until the cap matters, then escalating. Keeps the brand lime out of
	// a purely informational bar and makes the colour a real signal.
	const barFill: Record<UsageTone, string> = {
		neutral: "bg-foreground/35",
		warning: "bg-warning",
		critical: "bg-destructive",
	};
</script>

<section class="glass-card flex flex-col gap-4 rounded-xl p-5">
	<div class="flex items-center justify-between gap-2">
		<div class="flex items-center gap-2">
			<HardDrive class="size-4 text-muted-foreground" />
			<h2 class="text-sm font-semibold text-foreground">Workspace usage</h2>
		</div>
		<span class="rounded-full bg-foreground/5 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground ring-1 ring-inset ring-border-low/40">
			{view.planLabel}
		</span>
	</div>

	<!-- Storage -->
	<div>
		<div class="flex items-center justify-between text-xs">
			<span class="font-medium text-foreground">Storage</span>
			<span class="font-mono text-[11px] text-muted-foreground">
				{formatBytes(view.usedBytes)} / {view.storageLimit != null ? formatBytes(view.storageLimit) : "∞"}
			</span>
		</div>
		<div
			class="mt-2 h-1.5 overflow-hidden rounded-full bg-foreground/8"
			role="progressbar"
			aria-label="Storage used"
			aria-valuenow={view.storagePct}
			aria-valuemin={0}
			aria-valuemax={100}
		>
			<div
				class="h-full rounded-full transition-[width] duration-700 ease-[cubic-bezier(0.625,0.05,0,1)] {barFill[view.storageTone]}"
				style="width: {view.storagePct}%"
			></div>
		</div>
		<p class="mt-1.5 text-[11px] text-muted-foreground">
			{view.storageStatus}
		</p>
	</div>

	<!-- Active links -->
	<div>
		<div class="flex items-center justify-between text-xs">
			<span class="font-medium text-foreground">
				<Link2 class="-mt-0.5 mr-1 inline size-3 text-muted-foreground" />
				Active recasts
			</span>
			<span class="font-mono text-[11px] text-muted-foreground">
				{view.activeRecasts} / {view.linksLimit ?? "∞"}
			</span>
		</div>
		<div
			class="mt-2 h-1.5 overflow-hidden rounded-full bg-foreground/8"
			role="progressbar"
			aria-label="Active recasts used"
			aria-valuenow={view.linksPct}
			aria-valuemin={0}
			aria-valuemax={100}
		>
			<div
				class="h-full rounded-full transition-[width] duration-700 ease-[cubic-bezier(0.625,0.05,0,1)] {barFill[view.linksTone]}"
				style="width: {view.linksPct}%"
			></div>
		</div>
		<p class="mt-1.5 text-[11px] text-muted-foreground">
			{view.linksStatus}
		</p>
	</div>
</section>
