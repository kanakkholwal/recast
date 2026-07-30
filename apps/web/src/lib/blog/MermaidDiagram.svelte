<script lang="ts">
// Mermaid is ~500KB, so it is imported dynamically and only on mount. A post
// with no diagram never pays for it, and it stays out of the SSR pass (it
// wants a DOM). Until it resolves, the source renders as a code block, which
// is also what a reader with no JS sees.
import { prefersReducedMotion } from "$lib/motion-core";

let { source }: { source: string } = $props();

let svg = $state<string | null>(null);
let failed = $state(false);
let host = $state<HTMLDivElement | null>(null);

// Diagram ids must be unique per page or mermaid's internal <defs> collide.
let seq = 0;
const uid = `mermaid-${Math.random().toString(36).slice(2, 9)}-${seq++}`;

$effect(() => {
	let cancelled = false;
	// `source` is a dependency: a hot reload in dev swaps it.
	const diagram = source;

	void (async () => {
		try {
			const mermaid = (await import("mermaid")).default;
			mermaid.initialize({
				startOnLoad: false,
				// Diagrams are read as part of the article, so they inherit the
				// article's ink rather than mermaid's own palette.
				theme: "base",
				themeVariables: {
					fontFamily: 'ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, sans-serif',
					fontSize: "14px",
					primaryColor: "transparent",
					primaryTextColor: "currentColor",
					primaryBorderColor: "currentColor",
					lineColor: "currentColor",
					secondaryColor: "transparent",
					tertiaryColor: "transparent",
				},
				flowchart: { curve: "basis", htmlLabels: true },
				securityLevel: "strict",
				// Honour the OS setting; mermaid animates edges otherwise.
				...(prefersReducedMotion() ? { theme: "base" } : {}),
			});
			const rendered = await mermaid.render(uid, diagram);
			if (!cancelled) svg = rendered.svg;
		} catch (err) {
			// A malformed diagram must not take the article down with it; the
			// fallback below keeps the source readable.
			console.error("mermaid render failed", err);
			if (!cancelled) failed = true;
		}
	})();

	return () => {
		cancelled = true;
	};
});
</script>

{#if svg && !failed}
	<div class="mermaid" bind:this={host} role="img">
		<!-- Built from our own markdown at author time, rendered by mermaid in
		     `strict` mode, which strips scripts and inline handlers. -->
		<!-- eslint-disable-next-line svelte/no-at-html-tags -->
		{@html svg}
	</div>
{:else}
	<pre class="mermaid-fallback"><code>{source}</code></pre>
{/if}

<style>
	.mermaid {
		margin: 0 0 1.6em;
		padding: 1.25em;
		border: 1px solid var(--border);
		border-radius: 12px;
		background: color-mix(in oklab, var(--muted) 35%, transparent);
		/* Diagrams are wider than the prose column on narrow screens; scroll the
		   diagram rather than the page. */
		overflow-x: auto;
		color: var(--muted-foreground);
	}

	.mermaid :global(svg) {
		display: block;
		margin: 0 auto;
		max-width: 100%;
		height: auto;
	}

	/* Mermaid writes `fill`/`stroke` attributes from its theme; these take the
	   node text and edges back to the article's own tokens in both themes. */
	.mermaid :global(.nodeLabel),
	.mermaid :global(.edgeLabel),
	.mermaid :global(.label) {
		color: var(--foreground);
		fill: var(--foreground);
	}
	.mermaid :global(.edgeLabel) {
		background: var(--background);
	}
	.mermaid :global(.node rect),
	.mermaid :global(.node circle),
	.mermaid :global(.node polygon),
	.mermaid :global(.node path) {
		stroke: color-mix(in oklab, var(--primary) 55%, var(--border));
		fill: color-mix(in oklab, var(--muted) 60%, transparent);
	}
	.mermaid :global(.edgePath path),
	.mermaid :global(.flowchart-link) {
		stroke: var(--muted-foreground);
	}
	.mermaid :global(marker path) {
		fill: var(--muted-foreground);
		stroke: var(--muted-foreground);
	}

	.mermaid-fallback {
		margin: 0 0 1.6em;
		padding: 1.1em 1.25em;
		border: 1px solid var(--border);
		border-radius: 12px;
		overflow-x: auto;
		font-size: 0.9em;
		line-height: 1.6;
		background: color-mix(in oklab, var(--muted) 35%, transparent);
	}
</style>
