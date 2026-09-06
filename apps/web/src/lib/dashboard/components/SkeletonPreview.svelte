<script lang="ts">
// The shape of what Pro unlocks, with no numbers; heights are a fixed pattern so server and client render alike.
let { kind = "chart" }: { kind?: "chart" | "list" | "curve" } = $props();

const bars = [38, 62, 45, 80, 55, 70, 34, 88, 50, 66, 42, 76];
const rows = [82, 64, 47, 33];
</script>

{#if kind === "chart"}
	<div class="flex h-32 items-end gap-2">
		{#each bars as h, i (i)}
			<span class="flex-1 rounded-t-sm bg-foreground/25" style="height: {h}%"></span>
		{/each}
	</div>
{:else if kind === "curve"}
	<div class="h-32 w-full">
		<svg viewBox="0 0 100 40" preserveAspectRatio="none" class="h-full w-full">
			<path
				d="M0 2 C 18 6, 30 16, 46 22 S 74 33, 100 37 L100 40 L0 40 Z"
				class="fill-foreground/20"
			/>
			<path
				d="M0 2 C 18 6, 30 16, 46 22 S 74 33, 100 37"
				class="fill-none stroke-foreground/40"
				stroke-width="1.5"
			/>
		</svg>
	</div>
{:else}
	<ul class="space-y-3">
		{#each rows as w, i (i)}
			<li>
				<div class="flex items-center justify-between gap-3">
					<span class="h-2.5 w-24 rounded-full bg-foreground/20"></span>
					<span class="h-2.5 w-10 rounded-full bg-foreground/15"></span>
				</div>
				<div class="mt-1.5 h-1.5 overflow-hidden rounded-full bg-paper">
					<div class="h-full rounded-full bg-foreground/25" style="width: {w}%"></div>
				</div>
			</li>
		{/each}
	</ul>
{/if}
