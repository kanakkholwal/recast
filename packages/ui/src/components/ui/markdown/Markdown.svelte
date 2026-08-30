<script lang="ts" module>
import { marked } from "marked";
import DOMPurify from "dompurify";

// GFM is on by default; `breaks: false` keeps GitHub's newline semantics instead of turning every newline into a break.
marked.setOptions({ gfm: true, breaks: false });

// marked does not sanitize, so its output MUST go through DOMPurify before `{@html}`; the link-hardening hook registers once, in the browser.
let hookInstalled = false;
function ensureHook() {
	if (hookInstalled || typeof window === "undefined") return;
	DOMPurify.addHook("afterSanitizeAttributes", (node) => {
		if (node.tagName === "A") {
			node.setAttribute("target", "_blank");
			node.setAttribute("rel", "noopener noreferrer");
		}
	});
	hookInstalled = true;
}

export function renderMarkdown(source: string, inline: boolean): string {
	const src = source ?? "";
	const parsed = inline ? marked.parseInline(src) : marked.parse(src);
	// Async mode is never enabled, so the result is always a string; guard anyway against a future option flip.
	if (typeof parsed !== "string") return "";
	// DOMPurify needs a DOM, so return empty under SSR and let client hydration fill it in.
	if (typeof window === "undefined") return "";
	ensureHook();
	return DOMPurify.sanitize(parsed);
}
</script>

<script lang="ts">
  import { cn } from "@recast/ui/utils";

  interface Props {
    /** Raw markdown source. */
    source: string;
    /** Render inline (no block wrappers like `<p>`) — for single-line text. */
    inline?: boolean;
    class?: string;
  }

  let { source, inline = false, class: className }: Props = $props();

  const html = $derived(renderMarkdown(source, inline));
</script>

{#if inline}
  <span class={cn("markdown markdown--inline", className)}>{@html html}</span>
{:else}
  <div class={cn("markdown", className)}>{@html html}</div>
{/if}

<style>
  /* `{@html}` content escapes Svelte's scoping, so style it via `:global()` under the wrapper, using design tokens. */
  .markdown {
    color: var(--foreground);
    font-size: inherit;
    line-height: 1.65;
    overflow-wrap: anywhere;
  }

  .markdown :global(:first-child) {
    margin-top: 0;
  }
  .markdown :global(:last-child) {
    margin-bottom: 0;
  }

  .markdown :global(p) {
    margin: 0 0 0.75em;
  }

  .markdown :global(h1),
  .markdown :global(h2),
  .markdown :global(h3),
  .markdown :global(h4) {
    margin: 1.25em 0 0.5em;
    font-weight: 600;
    line-height: 1.3;
    letter-spacing: -0.01em;
    color: var(--foreground);
  }
  .markdown :global(h1) {
    font-size: 1.4em;
  }
  .markdown :global(h2) {
    font-size: 1.2em;
  }
  .markdown :global(h3) {
    font-size: 1.05em;
  }
  .markdown :global(h4) {
    font-size: 1em;
  }

  .markdown :global(strong) {
    font-weight: 650;
    color: var(--foreground);
  }
  .markdown :global(em) {
    font-style: italic;
  }
  .markdown :global(del) {
    text-decoration: line-through;
    color: var(--muted-foreground);
  }

  .markdown :global(a) {
    color: var(--primary);
    text-decoration: none;
    text-underline-offset: 2px;
  }
  .markdown :global(a:hover) {
    text-decoration: underline;
  }

  .markdown :global(ul),
  .markdown :global(ol) {
    margin: 0 0 0.75em;
    padding-left: 1.4em;
  }
  .markdown :global(ul) {
    list-style: disc;
  }
  .markdown :global(ol) {
    list-style: decimal;
  }
  .markdown :global(li) {
    margin: 0.2em 0;
  }
  .markdown :global(li::marker) {
    color: var(--muted-foreground);
  }
  /* GFM task lists */
  .markdown :global(li:has(> input[type="checkbox"])) {
    list-style: none;
    margin-left: -1.2em;
  }
  .markdown :global(input[type="checkbox"]) {
    margin-right: 0.4em;
    accent-color: var(--primary);
  }

  .markdown :global(code) {
    font-family:
      ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace;
    font-size: 0.85em;
    padding: 0.12em 0.4em;
    border-radius: 4px;
    background: color-mix(in oklab, var(--muted) 70%, transparent);
    color: var(--foreground);
  }
  .markdown :global(pre) {
    margin: 0 0 0.75em;
    padding: 0.85em 1em;
    border-radius: 8px;
    background: color-mix(in oklab, var(--muted) 60%, transparent);
    border: 1px solid var(--border);
    overflow-x: auto;
  }
  .markdown :global(pre code) {
    padding: 0;
    background: none;
    font-size: 0.85em;
    line-height: 1.6;
  }

  .markdown :global(blockquote) {
    margin: 0 0 0.75em;
    padding: 0.2em 0 0.2em 0.9em;
    border-left: 3px solid var(--border);
    color: var(--muted-foreground);
  }

  .markdown :global(hr) {
    margin: 1.25em 0;
    border: none;
    border-top: 1px solid var(--border);
  }

  /* GFM tables — the main thing the old hand-rolled parser dropped. */
  .markdown :global(table) {
    width: 100%;
    margin: 0 0 0.75em;
    border-collapse: collapse;
    font-size: 0.92em;
    display: block;
    overflow-x: auto;
  }
  .markdown :global(th),
  .markdown :global(td) {
    padding: 0.5em 0.75em;
    border: 1px solid var(--border);
    text-align: left;
    vertical-align: top;
  }
  .markdown :global(th) {
    font-weight: 600;
    background: color-mix(in oklab, var(--muted) 50%, transparent);
    color: var(--foreground);
  }
  .markdown :global(tbody tr:nth-child(even)) {
    background: color-mix(in oklab, var(--muted) 25%, transparent);
  }

  .markdown :global(img) {
    max-width: 100%;
    border-radius: 8px;
  }

  /* Inline variant: strip block spacing so it sits on one line. */
  .markdown--inline :global(p) {
    margin: 0;
    display: inline;
  }
</style>
