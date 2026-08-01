<script lang="ts">
/**
 * Phase line + bar + byte/ETA readout for a long upload.
 *
 * Both upload dialogs drew this by hand, and neither exposed it as a
 * progressbar — a screen reader got the phase text but never the percentage,
 * on the one operation in the app worth tracking.
 */
import { Spinner } from "@recast/ui/spinner";
import { cn } from "@recast/ui/utils";

interface Props {
	/** e.g. "Uploading… 42%" */
	phaseLabel: string;
	/** 0–100, or null while the phase has no measurable progress. */
	pct: number | null;
	/** True while bytes are moving, which is when the spinner earns its place. */
	active: boolean;
	failed?: boolean;
	/** e.g. "12.3 MB of 45.0 MB · ~40s left" */
	transferLabel?: string | null;
	/** Right-aligned control beside the readout (e.g. "Cancel upload"). */
	trailing?: import("svelte").Snippet;
}

let { phaseLabel, pct, active, failed = false, transferLabel = null, trailing }: Props = $props();

const indeterminate = $derived(active && pct == null);
</script>

<div class="space-y-2.5">
	<div class="flex items-center justify-between gap-2 text-xs">
		<span class={cn("font-medium", failed ? "text-destructive" : "text-foreground")}>
			{phaseLabel}
		</span>
		{#if active}
			<Spinner class="size-3.5 shrink-0 text-muted-foreground" />
		{/if}
	</div>

	{#if !failed}
		<div
			class="h-1.5 w-full overflow-hidden rounded-full bg-muted"
			role="progressbar"
			aria-valuemin={0}
			aria-valuemax={100}
			aria-valuenow={indeterminate ? undefined : (pct ?? 0)}
			aria-valuetext={transferLabel ?? phaseLabel}
			aria-label={phaseLabel}
		>
			{#if indeterminate}
				<div class="h-full w-1/3 rounded-full bg-primary motion-safe:animate-pulse"></div>
			{:else}
				<div
					class="h-full rounded-full bg-primary transition-[width] duration-200"
					style="width: {pct ?? 0}%"
				></div>
			{/if}
		</div>

		{#if transferLabel || trailing}
			<div class="flex items-center justify-between gap-2">
				<span class="text-[10px] font-medium tabular-nums text-muted-foreground">
					{transferLabel ?? ""}
				</span>
				{@render trailing?.()}
			</div>
		{/if}
	{/if}
</div>
