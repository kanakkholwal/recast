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

<article class="prose" class:reference={width === "reference"}>
	<DocviaContent {nodes} />
</article>

<style>
	/* The Renderer emits its own elements, so Svelte's scoped classes never land
	   on them. Style them through `:global()` under this wrapper, and take every
	   colour from a design token so the page tracks the active theme. */
	.prose {
		color: var(--foreground);
		font-size: 1.0625rem;
		line-height: 1.75;
		max-width: 68ch;
	}
	.prose.reference {
		max-width: 78ch;
	}

	.prose :global(> :first-child) {
		margin-top: 0;
	}
	.prose :global(> :last-child) {
		margin-bottom: 0;
	}

	.prose :global(p) {
		margin: 0 0 1.4em;
		text-wrap: pretty;
	}

	.prose :global(h2),
	.prose :global(h3),
	.prose :global(h4) {
		color: var(--foreground);
		font-weight: 600;
		letter-spacing: -0.015em;
		line-height: 1.25;
		text-wrap: balance;
		/* Clears the fixed navbar when a heading anchor is jumped to. */
		scroll-margin-top: 6rem;
	}
	.prose :global(h2) {
		margin: 2.4em 0 0.8em;
		font-size: 1.65em;
	}
	.prose :global(h3) {
		margin: 2em 0 0.6em;
		font-size: 1.3em;
	}
	.prose :global(h4) {
		margin: 1.8em 0 0.5em;
		font-size: 1.1em;
	}

	.prose :global(strong) {
		font-weight: 650;
		color: var(--foreground);
	}
	.prose :global(em) {
		font-style: italic;
	}

	.prose :global(a) {
		color: var(--primary);
		text-decoration: underline;
		text-decoration-thickness: 1px;
		text-underline-offset: 3px;
		text-decoration-color: color-mix(in oklab, var(--primary) 40%, transparent);
		transition: text-decoration-color 150ms ease;
	}
	.prose :global(a:hover) {
		text-decoration-color: var(--primary);
	}

	/* A file reference is a link to the source, so it reads as a link: primary,
	   underlined, and without the code chip's background competing with it. */
	.prose :global(a code) {
		background: none;
		padding: 0;
		color: inherit;
	}

	.prose :global(ul),
	.prose :global(ol) {
		margin: 0 0 1.4em;
		padding-left: 1.5em;
	}
	.prose :global(ul) {
		list-style: disc;
	}
	.prose :global(ol) {
		list-style: decimal;
	}
	.prose :global(li) {
		margin: 0.4em 0;
		padding-left: 0.2em;
	}
	.prose :global(li::marker) {
		color: var(--muted-foreground);
	}

	.prose :global(code) {
		font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace;
		font-size: 0.875em;
		padding: 0.15em 0.4em;
		border-radius: 5px;
		background: color-mix(in oklab, var(--muted) 70%, transparent);
		color: var(--foreground);
		overflow-wrap: anywhere;
	}
	/* Shiki bakes the highlighted markup (and its own background) into the block
	   at build time, so only the frame is ours. */
	.prose :global(pre) {
		margin: 0 0 1.6em;
		padding: 1.1em 1.25em;
		border-radius: 12px;
		border: 1px solid var(--border);
		overflow-x: auto;
		font-size: 0.9em;
		line-height: 1.6;
	}
	.prose :global(pre code) {
		padding: 0;
		background: none;
		font-size: inherit;
	}

	.prose :global(blockquote) {
		margin: 0 0 1.4em;
		padding: 0.3em 0 0.3em 1.1em;
		border-left: 3px solid color-mix(in oklab, var(--primary) 50%, transparent);
		color: var(--muted-foreground);
		font-style: italic;
	}

	.prose :global(hr) {
		margin: 2.5em 0;
		border: none;
		border-top: 1px solid var(--border);
	}

	.prose :global(img) {
		max-width: 100%;
		height: auto;
		border-radius: 12px;
		border: 1px solid var(--border);
	}

	.prose :global(table) {
		width: 100%;
		margin: 0 0 1.6em;
		border-collapse: collapse;
		font-size: 0.92em;
		display: block;
		overflow-x: auto;
	}
	.prose :global(th),
	.prose :global(td) {
		padding: 0.55em 0.8em;
		border: 1px solid var(--border);
		text-align: left;
		vertical-align: top;
	}
	.prose :global(th) {
		font-weight: 600;
		background: color-mix(in oklab, var(--muted) 50%, transparent);
	}
</style>
