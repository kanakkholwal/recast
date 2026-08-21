<script lang="ts">
import { cn } from "@recast/ui/utils";
import type { Snippet } from "svelte";

let {
	children,
	class: className = "",
	id,
	spacing = "default",
}: {
	children: Snippet;
	class?: string;
	id?: string;
	spacing?: "tight" | "default" | "loose" | "none";
} = $props();

const spacings = {
	none: "",
	tight: "py-12 md:py-16",
	default: "py-16 md:py-24",
	loose: "py-24 md:py-32",
};
</script>

<section {id} class={cn("relative w-full", spacings[spacing], className)}>
	{@render children()}
</section>
