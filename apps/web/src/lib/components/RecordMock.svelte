<script lang="ts">
	import { prefersReducedMotion } from "$lib/motion-core";
	import { Layout, MonitorPlay, Play, Search } from "@recast/icons";
	import { cn } from "@recast/ui/utils";

	// Command-palette mock for Step 1. The highlighted row cycles like keyboard
	// navigation to read as "live", with an enter hint on the active option.
	// Reduced motion pins it to the first row (no cycling).
	const options = [
		{ icon: MonitorPlay, label: "Record full screen" },
		{ icon: Layout, label: "Record region" },
		{ icon: Play, label: "Continue last project" },
	];

	const reduced = $derived(prefersReducedMotion());
	let active = $state(0);

	$effect(() => {
		if (reduced) {
			active = 0;
			return;
		}
		const id = setInterval(() => {
			// Pause while the tab is hidden so it never animates off-screen.
			if (!document.hidden) active = (active + 1) % options.length;
		}, 1800);
		return () => clearInterval(id);
	});
</script>

<div class="p-5">
	<div
		class="relative rounded-xl border border-border-low/60 bg-background/60 p-4 shadow-craft-inset"
	>
		<div
			class="flex items-center gap-3 rounded-lg border border-border-low/60 bg-background/80 px-3 py-2.5"
		>
			<Search class="size-4 text-muted-foreground" />
			<span class="text-sm font-medium text-foreground/85">Start a recording…</span>
			<span
				class="ml-auto rounded-md border border-border-low/60 bg-background px-1.5 py-0.5 font-mono text-[10px] font-semibold text-muted-foreground"
			>
				⌘ ⇧ R
			</span>
		</div>
		<div class="mt-3 space-y-1.5">
			{#each options as opt, i (opt.label)}
				{@const Icon = opt.icon}
				<div
					class={cn(
						"flex items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors duration-300",
						i === active ? "bg-foreground/8 text-foreground" : "text-muted-foreground",
					)}
				>
					<Icon class={cn("size-3.5", i === active ? "text-foreground" : "")} />
					<span class="font-medium">{opt.label}</span>
					{#if i === active}
						<span
							class="ml-auto rounded border border-border-low/60 bg-background px-1 py-0.5 font-mono text-[9px] text-muted-foreground"
						>
							↵
						</span>
					{/if}
				</div>
			{/each}
		</div>
	</div>
</div>
