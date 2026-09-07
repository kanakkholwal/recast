<script lang="ts">
import { cn } from "@recast/ui/utils";
import { Command as CommandPrimitive, useId } from "bits-ui";

let {
	ref = $bindable(null),
	class: className,
	children,
	heading,
	value,
	...restProps
}: CommandPrimitive.GroupProps & {
	heading?: string;
} = $props();
</script>

<CommandPrimitive.Group
	bind:ref
	data-slot="command-group"
	class={cn("text-foreground **:[[cmdk-group-heading]]:text-muted-foreground/70 overflow-hidden p-1 **:[[cmdk-group-heading]]:px-2 **:[[cmdk-group-heading]]:pt-2 **:[[cmdk-group-heading]]:pb-1 **:[[cmdk-group-heading]]:text-[10px] **:[[cmdk-group-heading]]:font-semibold **:[[cmdk-group-heading]]:uppercase **:[[cmdk-group-heading]]:tracking-[0.12em]", className)}
	value={value ?? heading ?? `-${useId()}`}
	{...restProps}
>
	{#if heading}
		<CommandPrimitive.GroupHeading
			class="text-muted-foreground/70 px-2 pt-2 pb-1 text-[10px] font-semibold uppercase tracking-[0.12em]"
		>
			{heading}
		</CommandPrimitive.GroupHeading>
	{/if}
	<CommandPrimitive.GroupItems {children} />
</CommandPrimitive.Group>
