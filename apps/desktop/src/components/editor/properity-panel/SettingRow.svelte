<script lang="ts">
import type { Snippet } from "svelte";

// The label-on-the-left, control-on-the-right row, hand-rolled 24 times across
// the properties panels. Beyond the duplication, none of those copies tied the
// visible label to its control, and several controls carried an aria-label that
// said something different from the label next to it (WCAG 2.5.3, Label in
// Name): "Show" read as "Words shown at once" to a screen reader, so voice
// control could not act on what the user saw.
interface Props {
	label: string;
	/** Extra context. Sits under the label, not in a `title` tooltip. */
	description?: string;
	/** Renders the control. Spread `props` onto it so the label names it. */
	children: Snippet<[{ "aria-labelledby": string }]>;
	/** Stacks the control under the label, for wide controls like sliders. */
	stacked?: boolean;
}

let { label, description, children, stacked = false }: Props = $props();

const id = $props.id();
</script>

<div
	class={stacked
		? "flex flex-col gap-1.5"
		: "flex items-center justify-between gap-3"}
>
	<div class="min-w-0">
		<span id="{id}-label" class="text-[11px] text-foreground">{label}</span>
		{#if description}
			<p class="text-[11px] leading-snug text-muted-foreground">{description}</p>
		{/if}
	</div>
	{@render children({ "aria-labelledby": `${id}-label` })}
</div>
