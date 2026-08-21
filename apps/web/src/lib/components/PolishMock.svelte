<script lang="ts">
import { prefersReducedMotion } from "$lib/motion-core";
import { Check, MousePointer2, Scissors, ZoomIn } from "@recast/icons";
import { cn } from "@recast/ui/utils";

// Auto-polish landing on a take: zoom snaps in, silence drops out, the cursor
// path straightens. Abstract shapes only, so it never ages with the real editor.
const reduced = $derived(prefersReducedMotion());

const EDITS = [
	{ at: 1500, icon: ZoomIn, label: "Smart zoom" },
	{ at: 3000, icon: Scissors, label: "Silence cut" },
	{ at: 4500, icon: MousePointer2, label: "Cursor smoothing" },
] as const;
const LOOP_MS = 7800;

let elapsed = $state(0);

$effect(() => {
	if (reduced) return;
	const id = setInterval(() => {
		if (!document.hidden) elapsed = (elapsed + 50) % LOOP_MS;
	}, 50);
	return () => clearInterval(id);
});

const applied = $derived(reduced ? 3 : EDITS.filter((e) => elapsed >= e.at).length);

// Stage coordinates are percentages, so the mock scales with its container.
const RAW = [
	[16, 78],
	[23, 66],
	[28, 70],
	[34, 57],
	[41, 60],
	[47, 48],
	[54, 52],
	[61, 41],
	[68, 44],
	[74, 34],
] as const;
const SMOOTH = [
	[16, 78],
	[30, 63],
	[45, 52],
	[60, 43],
	[74, 34],
] as const;

const toPath = (pts: readonly (readonly number[])[]) =>
	pts.map((p, i) => `${i === 0 ? "M" : "L"}${p[0]} ${p[1]}`).join(" ");

// Point at t along a polyline, so the cursor rides whichever path is live.
function pointAt(pts: readonly (readonly number[])[], t: number) {
	const spans = pts.slice(1).map((p, i) => Math.hypot(p[0] - pts[i][0], p[1] - pts[i][1]));
	const total = spans.reduce((a, b) => a + b, 0);
	let travelled = t * total;
	for (let i = 0; i < spans.length; i++) {
		if (travelled <= spans[i] || i === spans.length - 1) {
			const k = spans[i] === 0 ? 0 : Math.min(1, travelled / spans[i]);
			return {
				x: pts[i][0] + (pts[i + 1][0] - pts[i][0]) * k,
				y: pts[i][1] + (pts[i + 1][1] - pts[i][1]) * k,
			};
		}
		travelled -= spans[i];
	}
	return { x: pts[0][0], y: pts[0][1] };
}

const travel = $derived(reduced ? 0.62 : (elapsed % 2600) / 2600);
const cursor = $derived(pointAt(applied >= 3 ? SMOOTH : RAW, travel));

// 44 fixed bars; 30..37 are the silence the cut removes.
const BARS = [
	38, 62, 44, 78, 55, 90, 47, 70, 35, 82, 58, 96, 41, 66, 52, 88, 44, 74, 60, 92, 36, 68, 50, 84,
	46, 76, 58, 64, 42, 70, 12, 9, 14, 8, 11, 9, 13, 10, 54, 80, 46, 72, 38, 60,
];
const SILENT_FROM = 30;
const SILENT_TO = 37;
</script>

<div class="p-4">
	<!-- Stage. Same abstract desktop as the record mock so the two read as siblings. -->
	<div class="relative aspect-16/10 w-full overflow-hidden rounded-xl border border-border-low bg-paper">
		<div aria-hidden="true" class="absolute inset-0 p-3">
			<div class="flex h-full gap-2">
				<div class="h-full w-1/4 rounded-lg border border-border-low bg-card"></div>
				<div class="flex h-full flex-1 flex-col gap-2">
					<div class="h-1/3 rounded-lg border border-border-low bg-card"></div>
					<div class="flex-1 rounded-lg border border-border-low bg-card"></div>
				</div>
			</div>
		</div>

		<!-- Cursor path: the jittery take under the smoothed line it becomes. -->
		<svg
			aria-hidden="true"
			viewBox="0 0 100 100"
			preserveAspectRatio="none"
			class="absolute inset-0 size-full"
		>
			<path
				d={toPath(RAW)}
				fill="none"
				stroke="currentColor"
				stroke-width="1"
				vector-effect="non-scaling-stroke"
				class={cn(
					"text-border-strong transition-opacity duration-700 motion-reduce:transition-none",
					applied >= 3 ? "opacity-25" : "opacity-100",
				)}
			/>
			<path
				d={toPath(SMOOTH)}
				fill="none"
				stroke="currentColor"
				stroke-width="1.5"
				stroke-linecap="round"
				vector-effect="non-scaling-stroke"
				class={cn(
					"text-tag-lavender transition-opacity duration-700 motion-reduce:transition-none",
					applied >= 3 ? "opacity-100" : "opacity-0",
				)}
			/>
		</svg>

		<!-- Zoom region -->
		<div
			aria-hidden="true"
			class={cn(
				"absolute border border-tag-lavender transition-all duration-700 ease-[cubic-bezier(0.625,0.05,0,1)] motion-reduce:transition-none",
				applied >= 1 ? "scale-100 opacity-100" : "scale-105 opacity-0",
			)}
			style="left:14%;top:16%;width:44%;height:46%"
		>
			{#each ["-top-px -left-px", "-top-px -right-px", "-bottom-px -left-px", "-bottom-px -right-px"] as pos (pos)}
				<span class={cn("absolute size-1 bg-tag-lavender", pos)}></span>
			{/each}
			<span
				class="absolute -top-2 left-2 bg-paper px-1 text-caption font-medium text-tag-lavender tabular-nums"
			>
				1.6×
			</span>
		</div>

		<span
			aria-hidden="true"
			class="absolute -ml-1 -mt-1 size-2 rounded-full bg-foreground ring-2 ring-background"
			style={`left:${cursor.x}%;top:${cursor.y}%`}
		></span>
	</div>

	<!-- Lanes. Hairline rows, one edit each, matching the page's ruled grids. -->
	<div class="mt-3 divide-y divide-border-low border-y border-border-low">
		<div class="flex items-center gap-3 py-2">
			<span class="w-12 shrink-0 text-caption font-medium text-muted-foreground">Zoom</span>
			<div class="relative h-4 flex-1">
				<div
					class={cn(
						"absolute inset-y-0 rounded-[3px] border border-tag-lavender bg-tag-lavender/10 transition-opacity duration-700 motion-reduce:transition-none",
						applied >= 1 ? "opacity-100" : "opacity-0",
					)}
					style="left:14%;width:44%"
				></div>
			</div>
		</div>

		<div class="flex items-center gap-3 py-2">
			<span class="w-12 shrink-0 text-caption font-medium text-muted-foreground">Audio</span>
			<div class="relative h-4 flex-1">
				<div class="flex h-full items-center gap-px">
					{#each BARS as height, i (i)}
						{@const silent = i >= SILENT_FROM && i <= SILENT_TO}
						<span
							class={cn(
								"min-w-0 flex-1 rounded-full bg-border-strong transition-opacity duration-500 motion-reduce:transition-none",
								silent && applied >= 2 ? "opacity-0" : "opacity-100",
							)}
							style={`height:${height}%`}
						></span>
					{/each}
				</div>
				<!-- Cut seam where the silence was. -->
				<span
					class={cn(
						"absolute inset-y-0 border-l border-dashed border-tag-green transition-opacity duration-500 motion-reduce:transition-none",
						applied >= 2 ? "opacity-100" : "opacity-0",
					)}
					style={`left:${((SILENT_FROM + SILENT_TO) / 2 / BARS.length) * 100}%`}
				></span>
			</div>
			<span
				class={cn(
					"w-12 shrink-0 text-right text-caption tabular-nums text-tag-green transition-opacity duration-500 motion-reduce:transition-none",
					applied >= 2 ? "opacity-100" : "opacity-0",
				)}
			>
				-1.2s
			</span>
		</div>
	</div>

	<div class="mt-3 flex flex-wrap items-center gap-x-4 gap-y-1.5">
		{#each EDITS as edit, i (edit.label)}
			{@const on = applied > i}
			<span
				class={cn(
					"inline-flex items-center gap-1.5 text-caption transition-colors duration-500 motion-reduce:transition-none",
					on ? "text-foreground" : "text-muted-foreground",
				)}
			>
				{#if on}
					<Check class="size-3 text-tag-green" />
				{:else}
					{@const Icon = edit.icon}
					<Icon class="size-3" />
				{/if}
				{edit.label}
			</span>
		{/each}
		<span class="ml-auto text-caption text-muted-foreground">Applied automatically</span>
	</div>
</div>
