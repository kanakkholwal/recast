<script lang="ts">
	import { cn } from "@recast/ui/utils";
	import { IconSearch } from "@tabler/icons-svelte";
	import { Command as CommandPrimitive } from "bits-ui";
	import * as InputGroup from "../input-group";

	let {
		ref = $bindable(null),
		class: className,
		wrapperClass,
		value = $bindable(""),
		...restProps
	}: CommandPrimitive.InputProps & {
		wrapperClass?: string;
	} = $props();
</script>

<div data-slot="command-input-wrapper" class="p-1 pb-0">
	<InputGroup.Root
		class={cn(
			"bg-input/30 border-input/30 h-8! rounded-lg! shadow-none! *:data-[slot=input-group-addon]:pl-2!",
			wrapperClass,
		)}
	>
		<CommandPrimitive.Input
			data-slot="command-input"
			class={cn(
				"w-full text-sm outline-hidden disabled:cursor-not-allowed disabled:opacity-50",
				className,
			)}
			bind:ref
			{...restProps}
			bind:value
		/>
		<InputGroup.Addon>
			<IconSearch class="size-4 shrink-0 opacity-50" />
		</InputGroup.Addon>
	</InputGroup.Root>
</div>
