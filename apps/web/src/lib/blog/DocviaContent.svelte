<script lang="ts">
// Recursive walker over docvia's compiled node tree. Self-imports to recurse
// (Svelte 5's replacement for `<svelte:self>`).
import Self from "./DocviaContent.svelte";
import MermaidDiagram from "./MermaidDiagram.svelte";
import { mermaidSourceOf } from "./mermaid";
import { VOID_TAGS, type DocNodes } from "./render";

let { nodes }: { nodes: DocNodes } = $props();

const list = $derived(Array.isArray(nodes) ? nodes : [nodes]);
</script>

{#each list as node (node)}
	{@const diagram = mermaidSourceOf(node)}
	{#if diagram}
		<!-- A ```mermaid fence. Rendered client-side so mermaid stays out of the
		     SSR pass and off every page that has no diagram. -->
		<MermaidDiagram source={diagram} />
	{:else if node.kind === "text"}
		{node.value}
	{:else if node.kind === "html"}
		<!-- Build-time output (e.g. Shiki-highlighted code). It is our own content,
		     compiled on our machine, never user input, so there is nothing to
		     sanitize against here. -->
		<!-- eslint-disable-next-line svelte/no-at-html-tags -->
		{@html node.value}
	{:else if node.kind === "fragment"}
		<Self nodes={node.children ?? []} />
	{:else if node.kind === "element"}
		{#if VOID_TAGS.has(node.tag)}
			<svelte:element this={node.tag} {...node.props ?? {}} />
		{:else}
			<svelte:element this={node.tag} {...node.props ?? {}}>
				<Self nodes={node.children ?? []} />
			</svelte:element>
		{/if}
	{:else if node.kind === "component"}
		<!-- Component directives (`:::name`) would need a registry. No post uses one;
		     render the children so content is never silently dropped if one appears. -->
		<Self nodes={node.children ?? []} />
	{/if}
{/each}
