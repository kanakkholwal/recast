<script lang="ts">
// Mermaid is ~500KB, so it is imported dynamically and only on mount. A page
// with no diagram never pays for it, and it stays out of the SSR pass (it
// wants a DOM). Until it resolves, the source renders as a code block, which
// is also what a reader with no JS sees.

import { MERMAID_THEME_VARIABLES } from "./mermaid-theme";

let { source }: { source: string } = $props();

let svg = $state<string | null>(null);
let failed = $state(false);

// Diagram ids must be unique per page or mermaid's internal <defs> collide.
const uid = `mermaid-${Math.random().toString(36).slice(2, 9)}`;

$effect(() => {
	let cancelled = false;
	// `source` is a dependency: a hot reload in dev swaps it.
	const diagram = source;

	void (async () => {
		try {
			const mermaid = (await import("mermaid")).default;
			mermaid.initialize({
				startOnLoad: false,
				theme: "base",
				themeVariables: { ...MERMAID_THEME_VARIABLES },
				flowchart: { curve: "basis" },
				securityLevel: "strict",
			});
			const rendered = await mermaid.render(uid, diagram);
			if (!cancelled) svg = rendered.svg;
		} catch (err) {
			// A malformed diagram must not take the page down with it; the
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
	<div class="mermaid" role="img">
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
		border: 1px solid var(--color-border-low);
		border-radius: 12px;
		background: var(--color-paper);
		/* Diagrams are wider than the prose column on narrow screens; scroll the
		   diagram rather than the page. */
		overflow-x: auto;
	}

	.mermaid :global(svg) {
		display: block;
		margin: 0 auto;
		max-width: 100%;
		height: auto;
	}

	/* Mermaid writes fill and stroke attributes from its own palette; these take
	   every mark back to a design token, in both themes. */
	.mermaid :global(.nodeLabel),
	.mermaid :global(.edgeLabel),
	.mermaid :global(.label),
	.mermaid :global(.messageText),
	.mermaid :global(.loopText),
	.mermaid :global(text.actor) {
		color: var(--color-foreground);
		fill: var(--color-foreground);
	}
	.mermaid :global(.edgeLabel) {
		background: var(--color-paper);
	}
	.mermaid :global(.node rect),
	.mermaid :global(.node circle),
	.mermaid :global(.node polygon),
	.mermaid :global(.node path),
	.mermaid :global(rect.actor),
	.mermaid :global(.note) {
		stroke: var(--color-border-strong);
		fill: var(--color-card);
	}
	.mermaid :global(.cluster rect) {
		stroke: var(--color-border-low);
		fill: transparent;
	}
	.mermaid :global(.edgePath path),
	.mermaid :global(.flowchart-link),
	.mermaid :global(line.messageLine0),
	.mermaid :global(line.messageLine1),
	.mermaid :global(line.actor-line) {
		stroke: var(--color-muted-foreground);
	}
	.mermaid :global(marker path),
	.mermaid :global(.marker) {
		fill: var(--color-muted-foreground);
		stroke: var(--color-muted-foreground);
	}

	.mermaid-fallback {
		margin: 0 0 1.6em;
		padding: 1.1em 1.25em;
		border: 1px solid var(--color-border-low);
		border-radius: 12px;
		overflow-x: auto;
		font-size: 0.9em;
		line-height: 1.6;
		background: var(--color-paper);
	}
</style>
