<script lang="ts">
import { ContextMenu as ContextMenuPrimitive } from "bits-ui";

let {
	ref = $bindable(null),
	value = $bindable([]),
	...restProps
}: ContextMenuPrimitive.CheckboxGroupProps = $props();
</script>

<ContextMenuPrimitive.CheckboxGroup
	bind:ref
	bind:value
	data-slot="context-menu-checkbox-group"
	{...restProps}
/>
