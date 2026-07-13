/**
 * docvia's compiled output format (`RenderOutput` in `@docvia/renderer-core`).
 *
 * Mirrored here rather than imported because we render it ourselves: shipping
 * `@docvia/renderer-svelte` to the browser meant importing a package whose
 * `dist/` contains an uncompiled `Renderer.svelte`, which Vite externalizes for
 * SSR and hands to Node, dying with `ERR_UNKNOWN_FILE_EXTENSION` on every
 * article request in dev. The node tree is plain JSON with five cases, so
 * `DocviaContent.svelte` walks it directly. That keeps the renderer out of the
 * client bundle entirely and leaves docvia doing what it is actually good at:
 * compiling markdown to this tree at build time.
 */
export type DocNode =
	| {
			kind: "element";
			tag: string;
			props?: Record<string, unknown>;
			children?: DocNode[];
			id?: string;
	  }
	| { kind: "text"; value: string }
	| { kind: "html"; value: string }
	| {
			kind: "component";
			name: string;
			props?: Record<string, unknown>;
			children?: DocNode[];
			id: string;
	  }
	| { kind: "fragment"; children: DocNode[] };

/** What a compiled page's `content` holds. */
export type DocNodes = DocNode | DocNode[];

/**
 * HTML elements that may not have children. `<svelte:element>` throws at runtime
 * if you give one a child, so they are rendered self-closing.
 */
export const VOID_TAGS: ReadonlySet<string> = new Set([
	"area",
	"base",
	"br",
	"col",
	"embed",
	"hr",
	"img",
	"input",
	"link",
	"meta",
	"param",
	"source",
	"track",
	"wbr",
]);
