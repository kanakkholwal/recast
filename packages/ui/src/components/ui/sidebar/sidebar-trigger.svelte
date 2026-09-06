<script lang="ts">
import { Button } from "../button";
import { PanelLeft } from "@recast/icons";
import { cn } from "@recast/ui/utils";
import type { ComponentProps } from "svelte";
import { useSidebar } from "./context.svelte";

let {
	ref = $bindable(null),
	class: className,
	onclick,
	...restProps
}: ComponentProps<typeof Button> & {
	onclick?: (e: MouseEvent) => void;
} = $props();

const sidebar = useSidebar();
</script>

<Button
	bind:ref
	data-sidebar="trigger"
	data-slot="sidebar-trigger"
	variant="ghost"
	size="icon-sm"
	class={cn("cn-sidebar-trigger", className)}
	type="button"
	onclick={(e) => {
		onclick?.(e);
		sidebar.toggle();
	}}
	{...restProps}
>
	<PanelLeft  />
	<span class="sr-only">Toggle Sidebar</span>
</Button>
