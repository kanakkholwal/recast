import { render } from "svelte/server";
import ImageResponse from "takumi-js/response";
import OgImage from "$lib/components/OgImage.svelte";
import type { RequestHandler } from "./$types";

// Satoshi has no Fontsource package, so the vendored woff2 is inlined at build time rather than read off disk.
import satoshiUrl from "$lib/../../static/fonts/Satoshi-500.woff2?url";

const INTER_CDN =
	"https://cdn.jsdelivr.net/npm/@fontsource-variable/inter@5.3.0/files/inter-latin-wght-normal.woff2";

// Prod forces takumi's wasm renderer with base64-inlined bytes: the native addon fails to trace into a Vercel function, takumi's vite loader reads a client assets dir that isn't there, and 5 MB is over the Edge limit. Dev keeps the native path.
let wasmModule: Promise<Uint8Array> | undefined;
const resolveTakumiModule = () => {
	if (import.meta.env.DEV) return undefined;
	wasmModule ??= import("@takumi-rs/wasm/takumi_wasm_bg.wasm?url").then((m) => {
		const dataUri = m.default;
		return Buffer.from(dataUri.slice(dataUri.indexOf(",") + 1), "base64");
	});
	return wasmModule;
};

let cachedSatoshi: Promise<ArrayBuffer> | null = null;
const loadSatoshi = () => {
	// A data: URI after inlining, a real URL in dev; fetch reads both.
	cachedSatoshi ??= fetch(satoshiUrl).then((res) => {
		if (!res.ok) throw new Error(`Satoshi font load failed: ${res.status}`);
		return res.arrayBuffer();
	});
	return cachedSatoshi;
};

let cachedInter: Promise<ArrayBuffer> | null = null;
const loadInter = () => {
	cachedInter ??= fetch(INTER_CDN).then((res) => {
		if (!res.ok) throw new Error(`Inter font fetch failed: ${res.status}`);
		return res.arrayBuffer();
	});
	return cachedInter;
};

const clip = (value: string | null, max: number, fallback = "") => {
	if (!value) return fallback;
	const trimmed = value.trim();
	if (!trimmed) return fallback;
	return trimmed.length > max ? `${trimmed.slice(0, max - 1).trimEnd()}…` : trimmed;
};

export const GET: RequestHandler = ({ url }) => {
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

	// In prod this is the wasm module, skipping native-addon detection; in dev it is undefined and takumi goes native.
	const takumiModule = resolveTakumiModule();

	return new ImageResponse(`${head}${body}`, {
		width: 1200,
		height: 630,
		...(takumiModule ? { module: takumiModule } : {}),
		fonts: [
			{ name: "Satoshi", data: loadSatoshi },
			{ name: "Inter", data: loadInter },
		],
		headers: {
			"Cache-Control": "public, max-age=3600, s-maxage=86400, stale-while-revalidate=604800",
		},
	});
};
