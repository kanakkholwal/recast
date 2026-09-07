<script lang="ts">
import { Grid3x3, List } from "@recast/icons";
import { Segmented } from "@recast/ui/segmented";

interface Props {
	value: "grid" | "list";
}

let { value = $bindable("grid") }: Props = $props();
</script>

{#snippet gridIcon()}<Grid3x3 size={13} />{/snippet}
{#snippet listIcon()}<List size={13} />{/snippet}

<Segmented
	size="md"
	fill={false}
	segmentClass="w-8 px-0"
	options={[
		{ value: "grid", icon: gridIcon, title: "Grid view" },
		{ value: "list", icon: listIcon, title: "List view" },
	]}
	{value}
	onValueChange={(v) => (value = v)}
	aria-label="View"
/>
