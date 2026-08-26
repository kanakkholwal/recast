import { render } from "svelte/server";
import ImageResponse from "takumi-js/response";
import OgImage from "$lib/components/OgImage.svelte";
import type { RequestHandler } from "./$types";

const GEIST_CDN =
	"https://cdn.jsdelivr.net/npm/@fontsource-variable/geist@5.2.8/files/geist-latin-wght-normal.woff2";

// Renderer backend selection.
//
// In dev, takumi auto-detects the native `@takumi-rs/core` addon, which loads
// fine on a local machine — so we let it. In production on Vercel that native
// `.node` binary fails to trace into the serverless function (pnpm symlinks +
// @vercel/nft), so takumi throws at startup and `/api/og` returns a 500.
//
// For production we force takumi's bundled WebAssembly renderer via the `module`
// option, feeding it the wasm bytes directly. We deliberately DON'T use takumi's
// `@takumi-rs/wasm/vite` loader: it reads the wasm from a sibling `client/`
// assets dir, which exists under adapter-node but NOT inside a Vercel serverless
// function (client assets are served separately by the CDN) — so it 500s with
// "Unable to locate Takumi WASM asset for SSR". Vercel Edge isn't an option
// either: the 5 MB wasm exceeds the Edge bundle size limit.
//
// Instead we base64-inline the wasm into this server bundle via Vite's
// `build.assetsInlineLimit` (see vite.config). The bytes then travel *inside*
// the function — no filesystem, no CDN asset, no native addon. The import is
// dynamic + prod-gated so the dev server never evaluates the inlined data URI.
let wasmModule: Promise<Uint8Array> | undefined;
const resolveTakumiModule = () => {
	if (import.meta.env.DEV) return undefined;
	wasmModule ??= import("@takumi-rs/wasm/takumi_wasm_bg.wasm?url").then((m) => {
		const dataUri = m.default;
		return Buffer.from(dataUri.slice(dataUri.indexOf(",") + 1), "base64");
	});
	return wasmModule;
};

let cachedFont: Promise<ArrayBuffer> | null = null;
const loadGeist = () => {
	if (!cachedFont) {
		cachedFont = fetch(GEIST_CDN).then((res) => {
			if (!res.ok) throw new Error(`Geist font fetch failed: ${res.status}`);
			return res.arrayBuffer();
		});
	}
	return cachedFont;
};

const clip = (value: string | null, max: number, fallback = "") => {
	if (!value) return fallback;
	const trimmed = value.trim();
	if (!trimmed) return fallback;
	return trimmed.length > max ? `${trimmed.slice(0, max - 1).trimEnd()}…` : trimmed;
};

export const GET: RequestHandler = async ({ url }) => {
	const title = clip(url.searchParams.get("title"), 90, "Record. Polish. Share.");
	const description = clip(
		url.searchParams.get("description"),
		180,
		"Recast turns a raw screen capture into a polished, shareable demo. Smart auto-edits and a friendly timeline anyone can drive.",
	);
	const eyebrow = clip(url.searchParams.get("eyebrow"), 24);

	const { body, head } = render(OgImage, {
		props: { title, description, eyebrow },
	});

	// In prod this is the wasm module (skips native-addon auto-detection); in dev
	// it is undefined, so takumi uses the native renderer.
	const takumiModule = resolveTakumiModule();

	return new ImageResponse(`${head}${body}`, {
		width: 1200,
		height: 630,
		...(takumiModule ? { module: takumiModule } : {}),
		fonts: [
			{
				name: "Geist",
				data: loadGeist,
			},
		],
		headers: {
			"Cache-Control": "public, max-age=3600, s-maxage=86400, stale-while-revalidate=604800",
		},
	});
};
