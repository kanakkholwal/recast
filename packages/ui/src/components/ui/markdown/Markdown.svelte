<script lang="ts" module>
import DOMPurify from "dompurify";
import { marked } from "marked";

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
  <span class={className}>{@html html}</span>
{:else}
  <div class={className}>{@html html}</div>
{/if}
