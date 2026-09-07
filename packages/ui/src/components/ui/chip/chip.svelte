<script lang="ts">
/**
 * Compact pill for tags / filters. Two shapes:
 *   - default: a toggle button (use `selected` + `onclick`) — filter chips.
 *   - removable: a static pill with an inline remove button — assigned tags.
 * Optional `color` renders a leading dot (hex). Dependency-free (no icon lib).
 */
import { cn } from "@recast/ui/utils";

let {
	label,
	color = null,
	selected = false,
	removable = false,
	onclick,
	onremove,
	class: className,
}: {
	label: string;
	color?: string | null;
	selected?: boolean;
	removable?: boolean;
	onclick?: () => void;
	onremove?: () => void;
	class?: string;
} = $props();

const base =
	"inline-flex w-fit max-w-full shrink-0 items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs font-medium transition-colors";
const tone = $derived(
	selected
		? "border-primary/40 bg-primary/12 text-foreground"
		: "border-border-low/60 bg-foreground/3 text-muted-foreground hover:border-border hover:bg-foreground/8 hover:text-foreground",
);
</script>

{#if removable}
	<div data-slot="chip" class={cn(base, tone, className)}>
		{#if color}
			<span class="size-2 shrink-0 rounded-full" style="background:{color}"></span>
		{/if}
		<span class="truncate">{label}</span>
		<button
			type="button"
			aria-label={`Remove ${label}`}
			onclick={onremove}
			class="-mr-1 ml-0.5 grid size-4 shrink-0 place-items-center rounded-full leading-none text-muted-foreground/70 transition-colors hover:bg-foreground/10 hover:text-foreground"
		>
			×
		</button>
	</div>
{:else}
	<button
		type="button"
		data-slot="chip"
		aria-pressed={selected}
		{onclick}
		class={cn(base, tone, className)}
	>
		{#if color}
			<span class="size-2 shrink-0 rounded-full" style="background:{color}"></span>
		{/if}
		<span class="truncate">{label}</span>
	</button>
{/if}
