<script lang="ts">
import { Minus, Plus } from "@recast/icons";
import { cn } from "@recast/ui/utils";

// The −/+ pair that sits beside a NumberField, mirroring Premiere/Figma.
interface Props {
	onStep: (dir: 1 | -1) => void;
	disabled?: boolean;
	label?: string;
	class?: string;
}

let { onStep, disabled = false, label = "value", class: className }: Props = $props();

const btn =
	"flex h-8 w-7 items-center justify-center text-muted-foreground transition-colors hover:text-foreground disabled:pointer-events-none disabled:opacity-40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40";
</script>

<div
	class={cn(
		"inline-flex shrink-0 items-center rounded-md bg-muted/60 ring-1 ring-inset ring-border/40",
		className,
	)}
>
	<button
		type="button"
		{disabled}
		aria-label="Decrease {label}"
		class="{btn} rounded-l-md"
		onclick={() => onStep(-1)}
	>
		<Minus class="size-3" />
	</button>
	<div class="h-3.5 w-px bg-border/50" aria-hidden="true"></div>
	<button
		type="button"
		{disabled}
		aria-label="Increase {label}"
		class="{btn} rounded-r-md"
		onclick={() => onStep(1)}
	>
		<Plus class="size-3" />
	</button>
</div>
