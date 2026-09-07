<script lang="ts">
import DocviaContent from "./DocviaContent.svelte";
import type { DocNodes } from "./render";

interface Props {
	nodes: DocNodes;
	/** Caps the measure. Reference pages run wider than an article. */
	width?: "article" | "reference";
}

let { nodes, width = "article" }: Props = $props();
</script>

<article class={`prose ${width === "reference" ? "max-w-[78ch]" : "max-w-[68ch]"}`}>
	<DocviaContent {nodes} />
</article>
