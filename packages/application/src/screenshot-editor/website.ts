import { imageFromSrc } from "./image-input";
import type { EditorImage } from "./types";

/** Capture a live screenshot of a URL via Microlink's free, client-side API,
 * then inline it as a data URL so the export snapshot isn't CORS-tainted.
 * No API key; the free tier is rate-limited, so failures surface to the user. */
export async function captureWebsite(url: string): Promise<EditorImage> {
  const target = normalizeUrl(url);
  const api = `https://api.microlink.io/?url=${encodeURIComponent(target)}&screenshot=true&meta=false`;
  const res = await fetch(api);
  if (!res.ok) throw new Error(`screenshot service returned ${res.status}`);
  const json = (await res.json()) as { data?: { screenshot?: { url?: string } } };
  const shotUrl = json.data?.screenshot?.url;
  if (!shotUrl) throw new Error("could not capture that URL");

  const imgRes = await fetch(shotUrl);
  if (!imgRes.ok) throw new Error("could not download the screenshot");
  const blob = await imgRes.blob();
  return imageFromSrc(await blobToDataUrl(blob));
}

function normalizeUrl(url: string): string {
  const trimmed = url.trim();
  if (!trimmed) throw new Error("enter a website URL");
  return /^https?:\/\//i.test(trimmed) ? trimmed : `https://${trimmed}`;
}

function blobToDataUrl(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = () => reject(new Error("could not read the screenshot"));
    reader.readAsDataURL(blob);
  });
}
