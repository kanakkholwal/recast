<script lang="ts">
import { Controls, type Edge, type Node, SvelteFlow } from "@xyflow/svelte";
import { goto } from "$app/navigation";
import "@xyflow/svelte/dist/style.css";
import SubsystemNode from "./SubsystemNode.svelte";
import { SYSTEM_EDGES, SYSTEM_NODES } from "./system-map";

const nodeTypes = { subsystem: SubsystemNode };

// $state.raw, not $state: the flow reassigns these wholesale on interaction, and
// deep proxying every node would re-render the graph on each pan frame.
let nodes = $state.raw<Node[]>(
	SYSTEM_NODES.map((node) => ({
		id: node.id,
		type: "subsystem",
		position: { x: node.x, y: node.y },
		data: { label: node.label, runtime: node.runtime, phase: node.phase, slug: node.slug },
		draggable: false,
		selectable: node.slug !== null,
		ariaRole: "listitem",
		domAttributes: { "aria-label": `${node.label}, ${node.runtime}` },
	})),
);

let edges = $state.raw<Edge[]>(
	SYSTEM_EDGES.map((edge) => ({
		id: `${edge.source}-${edge.target}`,
		source: edge.source,
		target: edge.target,
		label: edge.label,
		animated: false,
	})),
);

function open({ node }: { node: Node }) {
	const slug = (node.data as { slug: string | null }).slug;
	if (slug) void goto(`/architecture/${slug}`);
}
</script>

<!-- The graph is a picture of the list below it, so it is hidden from assistive
     tech rather than exposed as an unnavigable node soup. -->
<div class="map surface-lg" aria-hidden="true">
	<SvelteFlow
		bind:nodes
		bind:edges
		{nodeTypes}
		fitView
		nodesDraggable={false}
		nodesConnectable={false}
		onnodeclick={open}
		zoomOnScroll={false}
		zoomOnDoubleClick={false}
		preventScrolling={false}
		minZoom={0.4}
		maxZoom={1.6}
	>
		<Controls showLock={false} orientation="horizontal" position="bottom-right" />
	</SvelteFlow>
</div>

<style>
	.map {
		height: 30rem;
		overflow: hidden;
	}

	/* xyflow paints its own canvas, controls, and edges from these variables, so
	   overriding them is what keeps the map inside the design system. */
	.map :global(.svelte-flow) {
		--xy-background-color: var(--color-paper);
		--xy-edge-stroke: var(--color-muted-foreground);
		--xy-edge-stroke-selected: var(--color-primary);
		--xy-edge-stroke-width: 1;
		--xy-edge-label-background-color: var(--color-card);
		--xy-edge-label-color: var(--color-muted-foreground);
		--xy-node-border-radius: 0.75rem;
		--xy-controls-button-background-color: var(--color-card);
		--xy-controls-button-background-color-hover: var(--color-muted);
		--xy-controls-button-color: var(--color-foreground);
		--xy-controls-button-color-hover: var(--color-foreground);
		--xy-controls-button-border-color: var(--color-border-low);
		--xy-controls-box-shadow: none;
		--xy-attribution-background-color: transparent;
	}

	/* The node's own border is the container; the wrapper must not draw a second. */
	.map :global(.svelte-flow__node) {
		border: none;
		background: none;
		padding: 0;
		width: auto;
		font-size: inherit;
	}

	.map :global(.svelte-flow__handle) {
		opacity: 0;
	}

	.map :global(.svelte-flow__edge-text) {
		font-size: 10px;
	}

	.map :global(.svelte-flow__controls) {
		border: 1px solid var(--color-border-low);
		border-radius: 9999px;
		overflow: hidden;
	}
</style>
