<script lang="ts">
import { Handle, type NodeProps, Position } from "@xyflow/svelte";
import type { MapPhase } from "./system-map";

type Data = {
	label: string;
	runtime: string;
	phase: MapPhase;
	slug: string | null;
};

let { data }: NodeProps = $props();

const node = $derived(data as Data);
</script>

<!-- Handles carry no interaction here: the map is a diagram, not an editor. -->
<Handle type="target" position={Position.Left} isConnectable={false} />

<div class="node">
	<span class="label">{node.label}</span>
	<span class="meta">
		<span class="dot" data-phase={node.phase}></span>
		{node.runtime}
	</span>
</div>

<Handle type="source" position={Position.Right} isConnectable={false} />

<style>
	/* Border-first like every container on the site: one hairline at full strength, with the phase read from the dot. */
	.node {
		display: flex;
		flex-direction: column;
		gap: 0.1875rem;
		width: 10.5rem;
		padding: 0.5rem 0.6875rem;
		border: 1px solid var(--color-border-low);
		border-radius: 0.75rem;
		background-color: var(--color-card);
		text-align: left;
	}

	.label {
		font-size: 0.8125rem;
		font-weight: 600;
		line-height: 1.3;
		letter-spacing: -0.011em;
		color: var(--color-foreground);
	}

	.meta {
		display: inline-flex;
		align-items: center;
		gap: 0.375rem;
		font-size: 0.6875rem;
		line-height: 1.4;
		color: var(--color-muted-foreground);
	}

	.dot {
		width: 0.3125rem;
		height: 0.3125rem;
		border-radius: 9999px;
		background-color: var(--phase-hue);
	}

	/* One hue per phase, matching the landing page's spine; an artifact is neutral, since a file is not a step. */
	.dot[data-phase="record"] {
		--phase-hue: var(--color-tag-tangerine);
	}
	.dot[data-phase="polish"] {
		--phase-hue: var(--color-tag-lavender);
	}
	.dot[data-phase="share"] {
		--phase-hue: var(--color-tag-green);
	}
	.dot[data-phase="artifact"] {
		--phase-hue: var(--color-border-strong);
	}
</style>
