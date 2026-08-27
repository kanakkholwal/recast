<script lang="ts" module>
import { marked } from "marked";
import DOMPurify from "dompurify";

// GFM is on by default in marked (tables, strikethrough, autolinks, task
// lists). `breaks: false` keeps GitHub's "two spaces / blank line" newline
// semantics rather than turning every `\n` into a `<br>`.
marked.setOptions({ gfm: true, breaks: false });

// Release bodies come from the GitHub API and can contain raw HTML, so the
// marked output MUST be sanitized before it reaches `{@html}` — marked does
// not sanitize (the maintainers explicitly defer to DOMPurify). Register the
// link-hardening hook once; it only runs in the browser (DOMPurify needs a
// DOM, which both consumers — the Tauri webview and the client-only web
// changelog — have).
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
	// We never enable marked's async mode, so the result is always a string;
	// guard anyway so a future option flip can't inject a `[object Promise]`.
	if (typeof parsed !== "string") return "";
	// DOMPurify needs a DOM. Under SSR there is none — return empty and let
	// client hydration fill it in (both current consumers render client-side).
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
  /* `{@html}` content isn't touched by Svelte's style scoping, so style it via
     `:global()` nested under the scoped wrapper. All colours come from the
     design tokens so it tracks the active theme. */
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
