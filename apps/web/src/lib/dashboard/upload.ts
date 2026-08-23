/**
 * Browser-side cloud upload, the web counterpart to the desktop's
 * "Share to Cloud" flow. Drives the same three server endpoints:
 *
 *   1. POST /api/uploads/init      → reserves a draft recast + signed PUT URLs
 *   2. PUT  <signed video url>     → uploads the MP4 (with byte progress)
 *   2b PUT  <signed poster url>    → uploads a WebP frame (best-effort)
 *   3. POST /api/uploads/complete  → HEAD-verifies + publishes
 *   4. POST /api/recasts/{id}/share → mints a public link
 *
 * Only web-ready `.mp4` / `.webm` are accepted. A `.recast` project can't be
 * turned into a shareable video in the browser, that needs the native render
 * pipeline, and its inner recording is the raw, unedited source.
 */

import { browser } from "$app/environment";

export type UploadPhase = "preparing" | "uploading" | "finalizing" | "sharing";

export interface UploadHandlers {
	workspaceId?: string;
	onPhase?: (phase: UploadPhase) => void;
	/** Byte progress 0–100 during the video PUT. */
	onProgress?: (pct: number) => void;
	/** Aborts the in-flight PUT. A long upload the user can't stop is a trap. */
	signal?: AbortSignal;
	share?: ShareOptions;
	/**
	 * When `false`, upload + publish only and skip minting the share link, the
	 * caller creates it afterwards via `createRecastShare`. Powers the
	 * step-by-step dialog that configures sharing *after* the upload finishes.
	 * Defaults to `true` (the quick-share behaviour home/library rely on).
	 */
	autoShare?: boolean;
	/**
	 * Pre-probed media supplied by the caller (the upload dialog), so the file's
	 * video element is loaded and sampled once. When present, the upload skips its
	 * own metadata read + poster capture and uses these values directly.
	 */
	media?: ProbedMedia;
}

export interface ProbedMedia {
	durationSec: number;
	width?: number;
	height?: number;
	/** WebP cover frame the caller already picked, or null for no poster. */
	posterBlob?: Blob | null;
}

export interface UploadResult {
	recastId: string;
	slug: string;
	shareUrl: string;
}

export type ShareVisibility = "private" | "workspace" | "selected" | "public";

export interface ShareOptions {
	visibility: ShareVisibility;
	password?: string;
	expiresAt?: string | null;
	commentsEnabled?: boolean;
	invitees?: Array<{ email: string; role: "viewer" | "commenter" }>;
}

interface SignedEnvelope {
	method: string;
	url: string;
	headers?: Record<string, string>;
}

/** Accept attribute + the guard below, keep them in sync. */
export const UPLOAD_ACCEPT = "video/mp4,video/webm,.mp4,.webm";

export function isUploadableVideo(file: File): boolean {
	return (
		file.type === "video/mp4" || file.type === "video/webm" || /\.(mp4|webm)$/i.test(file.name)
	);
}

/** The MIME we upload the file under, mirrored on `/init` (to sign the PUT)
 *  and on the PUT itself, so the signed content-type matches the bytes. */
export function uploadContentType(file: File): "video/mp4" | "video/webm" {
	if (file.type === "video/webm" || /\.webm$/i.test(file.name)) return "video/webm";
	return "video/mp4";
}

/** Header/button/progress label for the current upload phase. */
export function uploadPhaseLabel(phase: UploadPhase, pct: number): string {
	switch (phase) {
		case "uploading":
			return `Uploading ${pct}%`;
		case "finalizing":
			return "Finalizing…";
		case "sharing":
			return "Creating link…";
		default:
			return "Preparing…";
	}
}

// ── error mapping ─────────────────────────────────────────────────────

type Denial = { reason?: string };

function denialMessage(denial: Denial | undefined, fallback: string): string {
	switch (denial?.reason) {
		case "storage_over_cap":
			return "You're out of cloud storage. Upgrade or free up space.";
		case "active_recasts_over_cap":
			return "You've hit your active recast limit. Delete one or upgrade.";
		case "duration_over_cap":
			return "This recording is longer than your plan allows for cloud sharing.";
		case "resolution_over_cap":
			return "Your plan caps cloud sharing at 720p. Upload a 720p export, or upgrade for HD.";
		case "upload_missing":
			return "The upload didn't arrive, please try again.";
		case "empty_upload":
			return "That file came through empty, please try again.";
		default:
			return fallback;
	}
}

async function readJson(res: Response): Promise<Record<string, unknown> | null> {
	try {
		return (await res.json()) as Record<string, unknown>;
	} catch {
		return null;
	}
}

// ── media probing + poster capture ────────────────────────────────────

/** Poster encode width, matches the "replace cover" flow so covers are uniform. */
const POSTER_MAX_WIDTH = 960;

export function loadVideoElement(url: string): Promise<HTMLVideoElement> {
	return new Promise((resolve, reject) => {
		const v = document.createElement("video");
		v.preload = "auto";
		v.muted = true;
		v.playsInline = true;
		// Attach off-screen (not fully detached): a detached <video> doesn't
		// reliably decode frames, so canvas capture comes back blank/failed and
		// the recast ends up with no cover. A 2px rendered element decodes fine.
		v.style.cssText =
			"position:fixed;left:-9999px;top:0;width:2px;height:2px;opacity:0;pointer-events:none;";
		let settled = false;
		const finish = (ok: boolean) => {
			if (settled) return;
			settled = true;
			clearTimeout(timer);
			v.onloadeddata = null;
			v.onerror = null;
			if (ok) resolve(v);
			else {
				v.remove();
				reject(new Error("Couldn't read this video file."));
			}
		};
		// Resolve on `loadeddata` (a frame is available), not just metadata, so
		// the first seek+draw has something to capture.
		v.onloadeddata = () => finish(true);
		v.onerror = () => finish(false);
		const timer = setTimeout(() => finish(false), 15000);
		document.body.appendChild(v);
		v.src = url;
	});
}

/** Detach + release a video element loaded via `loadVideoElement`. */
export function releaseVideoElement(video: HTMLVideoElement): void {
	video.removeAttribute("src");
	video.load();
	video.remove();
}

function seekTo(video: HTMLVideoElement, time: number): Promise<void> {
	return new Promise((resolve) => {
		// Resolve on `seeked` (fires reliably for a detached/offscreen video), with
		// a safety timeout so a seek that never completes can never stall the
		// upload. (Do NOT gate this on requestVideoFrameCallback: it only fires
		// when a frame is composited, which a hidden, non-playing element never
		// does, so it would hang here forever.)
		let settled = false;
		const finish = () => {
			if (settled) return;
			settled = true;
			clearTimeout(timer);
			video.removeEventListener("seeked", finish);
			resolve();
		};
		video.addEventListener("seeked", finish);
		const timer = setTimeout(finish, 2000);
		try {
			video.currentTime = time;
		} catch {
			finish();
		}
	});
}

/** Encode the video's current frame as a downscaled WebP (null on failure). */
export async function captureFrameWebp(
	video: HTMLVideoElement,
	timeSec: number,
): Promise<Blob | null> {
	const w = video.videoWidth;
	const h = video.videoHeight;
	if (!w || !h) return null;
	await seekTo(video, Math.max(0, timeSec));
	const scaleW = Math.min(POSTER_MAX_WIDTH, w);
	const scaleH = Math.max(1, Math.round(h * (scaleW / w)));
	const canvas = document.createElement("canvas");
	canvas.width = scaleW;
	canvas.height = scaleH;
	const ctx = canvas.getContext("2d");
	if (!ctx) return null;
	ctx.drawImage(video, 0, 0, scaleW, scaleH);
	return await new Promise<Blob | null>((resolve) =>
		canvas.toBlob((b) => resolve(b), "image/webp", 0.82),
	);
}

/** Seek and paint the frame at `timeSec` into a canvas (drives the scrubber). */
export async function renderFrameToCanvas(
	video: HTMLVideoElement,
	canvas: HTMLCanvasElement,
	timeSec: number,
): Promise<void> {
	if (!video.videoWidth || !canvas.width) return;
	await seekTo(video, Math.max(0, timeSec));
	const ctx = canvas.getContext("2d");
	ctx?.drawImage(video, 0, 0, canvas.width, canvas.height);
}

/**
 * Score a tiny sampled frame for poster-worthiness. Rejects black/blank frames
 * (a fixed-timestamp grab lands on an intro fade or empty screen most of the
 * time) and rewards detail + colour, so we pick a frame that actually shows the
 * content. Cheap: runs on a ~64×36 downscale.
 */
function scoreFrame(img: ImageData): { value: number; usable: boolean } {
	const { data } = img;
	const n = data.length / 4;
	let sum = 0;
	let sumSq = 0;
	let rgSum = 0;
	let rgSq = 0;
	let ybSum = 0;
	let ybSq = 0;
	for (let i = 0; i < data.length; i += 4) {
		const r = data[i];
		const g = data[i + 1];
		const b = data[i + 2];
		const lum = 0.299 * r + 0.587 * g + 0.114 * b;
		sum += lum;
		sumSq += lum * lum;
		const rg = r - g;
		const yb = 0.5 * (r + g) - b;
		rgSum += rg;
		rgSq += rg * rg;
		ybSum += yb;
		ybSq += yb * yb;
	}
	const meanLum = sum / n;
	const stdLum = Math.sqrt(Math.max(0, sumSq / n - meanLum * meanLum));
	const rgMean = rgSum / n;
	const ybMean = ybSum / n;
	const rgStd = Math.sqrt(Math.max(0, rgSq / n - rgMean * rgMean));
	const ybStd = Math.sqrt(Math.max(0, ybSq / n - ybMean * ybMean));
	// Hasler–Süsstrunk colourfulness.
	const colourfulness =
		Math.sqrt(rgStd * rgStd + ybStd * ybStd) + 0.3 * Math.sqrt(rgMean * rgMean + ybMean * ybMean);

	// Reject near-black, near-white, and flat/blank (no luminance spread).
	const usable = meanLum > 18 && meanLum < 245 && stdLum > 8;
	// Mid-brightness bonus so we don't favour blown-out or crushed frames.
	const midBonus = (1 - Math.abs(meanLum - 128) / 128) * 20;
	const value = stdLum + colourfulness * 0.8 + midBonus;
	return { value, usable };
}

/**
 * Pick the best cover frame by sampling several timestamps across the middle of
 * the clip (skipping intro/outro) and scoring each. Returns the winning frame as
 * a WebP plus its timestamp, or null if nothing decodes. Best-effort, a recast
 * with no poster falls back to the generated placeholder.
 */
export async function pickBestPosterFrame(
	video: HTMLVideoElement,
	samples = 5,
): Promise<{ blob: Blob; timeSec: number } | null> {
	const w = video.videoWidth;
	const h = video.videoHeight;
	if (!w || !h) return null;

	const duration = video.duration || 0;
	const times: number[] = [];
	if (duration > 0.2) {
		const lo = duration * 0.1;
		const hi = duration * 0.9;
		const count = Math.max(1, samples);
		if (count === 1) times.push((lo + hi) / 2);
		else for (let i = 0; i < count; i++) times.push(lo + ((hi - lo) * i) / (count - 1));
	} else {
		times.push(0);
	}

	const sc = document.createElement("canvas");
	sc.width = 64;
	sc.height = Math.max(1, Math.round(64 * (h / w)));
	const sctx = sc.getContext("2d", { willReadFrequently: true });
	if (!sctx) return null;

	let bestUsable: { time: number; value: number } | null = null;
	let bestAny: { time: number; value: number } | null = null;
	for (const t of times) {
		await seekTo(video, t);
		try {
			sctx.drawImage(video, 0, 0, sc.width, sc.height);
			const score = scoreFrame(sctx.getImageData(0, 0, sc.width, sc.height));
			if (!bestAny || score.value > bestAny.value) bestAny = { time: t, value: score.value };
			if (score.usable && (!bestUsable || score.value > bestUsable.value))
				bestUsable = { time: t, value: score.value };
		} catch {
			// Tainted/undecodable frame, skip it.
		}
	}

	const chosen = bestUsable ?? bestAny;
	if (!chosen) return null;
	const blob = await captureFrameWebp(video, chosen.time);
	return blob ? { blob, timeSec: chosen.time } : null;
}

/** Thrown when the caller aborts. Not a failure: the user asked for it. */
export class UploadCancelled extends Error {
	constructor() {
		super("Upload cancelled.");
		this.name = "UploadCancelled";
	}
}

// ── signed PUT with progress (fetch has no upload progress) ────────────

function putWithProgress(
	envelope: SignedEnvelope,
	body: Blob,
	contentTypeFallback: string,
	onProgress?: (pct: number) => void,
	signal?: AbortSignal,
): Promise<number> {
	return new Promise((resolve, reject) => {
		if (signal?.aborted) {
			reject(new UploadCancelled());
			return;
		}
		const xhr = new XMLHttpRequest();
		xhr.open("PUT", envelope.url);

		const onAbort = () => xhr.abort();
		signal?.addEventListener("abort", onAbort, { once: true });
		const done = () => signal?.removeEventListener("abort", onAbort);

		const headers = envelope.headers ?? {};
		const hasContentType = Object.keys(headers).some((k) => k.toLowerCase() === "content-type");
		for (const [k, v] of Object.entries(headers)) xhr.setRequestHeader(k, v);
		// Presigned PUTs sign the content-type; match what /init signed when the
		// envelope didn't carry it explicitly.
		if (!hasContentType) xhr.setRequestHeader("Content-Type", contentTypeFallback);

		if (onProgress) {
			xhr.upload.onprogress = (e) => {
				if (e.lengthComputable) onProgress(Math.round((e.loaded / e.total) * 100));
			};
		}
		xhr.onload = () => {
			done();
			resolve(xhr.status);
		};
		xhr.onerror = () => {
			done();
			reject(new Error("Upload failed, check your connection."));
		};
		xhr.onabort = () => {
			done();
			reject(new UploadCancelled());
		};
		xhr.send(body);
	});
}

// ── main flow ──────────────────────────────────────────────────────────

export async function uploadRecastFile(
	file: File,
	handlers: UploadHandlers = {},
): Promise<UploadResult> {
	if (!browser) throw new Error("Upload can only run in the browser.");
	if (!isUploadableVideo(file)) {
		throw new Error("Only .mp4 or .webm video files can be uploaded here.");
	}

	const contentType = uploadContentType(file);
	handlers.onPhase?.("preparing");

	// Metadata + cover frame: reuse the caller's pre-probed media when present
	// (the dialog loads the file's <video> once for its own preview + scrubber),
	// otherwise load the video and auto-pick a cover here.
	let objectUrl: string | null = null;
	let video: HTMLVideoElement | null = null;
	let durationSec: number;
	let width: number | undefined;
	let height: number | undefined;
	let posterBlob: Blob | null;

	try {
		if (handlers.media) {
			durationSec = Math.max(0, Math.round(handlers.media.durationSec || 0));
			width = handlers.media.width || undefined;
			height = handlers.media.height || undefined;
			posterBlob = handlers.media.posterBlob ?? null;
		} else {
			objectUrl = URL.createObjectURL(file);
			video = await loadVideoElement(objectUrl);
			durationSec = Math.max(0, Math.round(video.duration || 0));
			width = video.videoWidth || undefined;
			height = video.videoHeight || undefined;
			posterBlob = (await pickBestPosterFrame(video))?.blob ?? null;
		}
		const title = file.name.replace(/\.[^.]+$/, "") || "Untitled recast";

		// 1. init
		const initRes = await fetch("/api/uploads/init", {
			method: "POST",
			headers: { "content-type": "application/json" },
			body: JSON.stringify({
				workspaceId: handlers.workspaceId,
				title,
				durationSec,
				sizeBytes: file.size,
				width,
				height,
				contentType,
			}),
		});
		const init = await readJson(initRes);
		if (!initRes.ok || init?.ok === false) {
			throw new Error(
				denialMessage(
					init?.denial as Denial | undefined,
					(init?.message as string) ?? "Couldn't start the upload.",
				),
			);
		}
		const videoUpload = init?.upload as SignedEnvelope | undefined;
		const posterUpload = init?.posterUpload as SignedEnvelope | undefined;
		const recastId = init?.recastId as string;
		if (!videoUpload || videoUpload.method?.toUpperCase() !== "PUT") {
			throw new Error("This storage provider isn't supported by the web uploader yet.");
		}

		// 2. PUT the video
		handlers.onPhase?.("uploading");
		handlers.signal?.throwIfAborted();
		const status = await putWithProgress(
			videoUpload,
			file,
			contentType,
			handlers.onProgress,
			handlers.signal,
		);
		if (status < 200 || status >= 300) {
			throw new Error(`Upload rejected (${status}).`);
		}

		// 2b. PUT the poster (best-effort)
		let hasPoster = false;
		if (posterBlob && posterUpload?.method?.toUpperCase() === "PUT") {
			try {
				const ps = await putWithProgress(posterUpload, posterBlob, "image/webp");
				hasPoster = ps >= 200 && ps < 300;
			} catch {
				hasPoster = false;
			}
		}

		// 3. complete
		handlers.onPhase?.("finalizing");
		const compRes = await fetch("/api/uploads/complete", {
			method: "POST",
			headers: { "content-type": "application/json" },
			body: JSON.stringify({ recastId, width, height, durationSec, hasPoster }),
		});
		const comp = await readJson(compRes);
		if (!compRes.ok || comp?.ok === false) {
			throw new Error(
				denialMessage(
					comp?.denial as Denial | undefined,
					denialMessage(
						{ reason: comp?.reason as string | undefined },
						(comp?.message as string) ?? "Couldn't finalize the upload.",
					),
				),
			);
		}

		// 4. share (public link, matching the desktop "Share to Cloud" default).
		//    Skipped when the caller configures + mints the link afterwards.
		if (handlers.autoShare === false) {
			return { recastId, slug: "", shareUrl: "" };
		}
		handlers.onPhase?.("sharing");
		const share = handlers.share ?? { visibility: "public" };
		const { slug, shareUrl } = await createRecastShare(recastId, share);
		return { recastId, slug, shareUrl };
	} finally {
		if (objectUrl) URL.revokeObjectURL(objectUrl);
		if (video) releaseVideoElement(video);
	}
}

/**
 * Mint a public share link for an already-uploaded recast. Split out of
 * `uploadRecastFile` so a caller can gather the viewer's sharing choices
 * *after* the file is published, then create the link in one step.
 */
export async function createRecastShare(
	recastId: string,
	share: ShareOptions,
): Promise<{ slug: string; shareUrl: string }> {
	const shareRes = await fetch(`/api/recasts/${recastId}/share`, {
		method: "POST",
		headers: { "content-type": "application/json" },
		body: JSON.stringify(share),
	});
	const shareData = await readJson(shareRes);
	if (!shareRes.ok || !shareData?.slug) {
		throw new Error((shareData?.message as string) ?? "Couldn't create a share link.");
	}
	return {
		slug: shareData.slug as string,
		shareUrl: shareData.shareUrl as string,
	};
}
