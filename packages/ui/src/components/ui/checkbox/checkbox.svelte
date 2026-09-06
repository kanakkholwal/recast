<script lang="ts">
import { Checkbox as CheckboxPrimitive } from "bits-ui";
import { cn, type WithoutChildrenOrChild } from "@recast/ui/utils";
import { Check } from "@recast/icons";
import { Minus } from "@recast/icons";

let {
	ref = $bindable(null),
	checked = $bindable(false),
	indeterminate = $bindable(false),
	class: className,
	...restProps
}: WithoutChildrenOrChild<CheckboxPrimitive.RootProps> = $props();
</script>

<!--
  Styling keys off `data-state`, which is what bits-ui actually emits
  ("checked" | "unchecked" | "indeterminate"). The previous `data-checked:`
  variants compiled to `[data-checked]` and matched nothing, so a ticked box
  rendered identically to an empty one.

  Unchecked uses --border-control (3.64:1 light / 3.11:1 dark on --card) instead
  of --input (1.17:1), and carries an opaque fill so the box stays readable on a
  translucent surface like .glass-card.
-->
<CheckboxPrimitive.Root
	bind:ref
	data-slot="checkbox"
	class={cn(
		"peer relative flex size-4 shrink-0 items-center justify-center rounded-[4px] border outline-none transition-colors",
		"border-border-control bg-background dark:bg-input",
		"data-[state=unchecked]:hover:border-foreground/45",
		"data-[state=checked]:border-primary data-[state=checked]:bg-primary data-[state=checked]:text-primary-foreground",
		"data-[state=indeterminate]:border-primary data-[state=indeterminate]:bg-primary data-[state=indeterminate]:text-primary-foreground",
		"focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-3",
		"aria-invalid:border-destructive aria-invalid:ring-destructive/20 aria-invalid:ring-3 dark:aria-invalid:ring-destructive/40",
		// Grows the pointer target past the 16px box without affecting layout.
		"after:absolute after:-inset-x-3 after:-inset-y-2",
		"group-has-disabled/field:opacity-50 disabled:cursor-not-allowed disabled:opacity-50",
		className
	)}
	bind:checked
	bind:indeterminate
	{...restProps}
>
	{#snippet children({ checked, indeterminate })}
		<div
			data-slot="checkbox-indicator"
			class="[&>svg]:size-3.5 grid place-content-center text-current transition-none"
		>
			{#if indeterminate}
				<Minus stroke={3} />
			{:else if checked}
				<Check stroke={3} />
			{/if}
		</div>
	{/snippet}
</CheckboxPrimitive.Root>
