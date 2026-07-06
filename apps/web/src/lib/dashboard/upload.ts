/**
 * Browser-side cloud upload — the web counterpart to the desktop's
 * "Share to Cloud" flow. Drives the same three server endpoints:
 *
 *   1. POST /api/uploads/init      → reserves a draft recast + signed PUT URLs
 *   2. PUT  <signed video url>     → uploads the MP4 (with byte progress)
 *   2b PUT  <signed poster url>    → uploads a WebP frame (best-effort)
 *   3. POST /api/uploads/complete  → HEAD-verifies + publishes
 *   4. POST /api/recasts/{id}/share → mints a public link
 *
 * Only web-ready `.mp4` / `.webm` are accepted. A `.recast` project can't be
 * turned into a shareable video in the browser — that needs the native render
 * pipeline — and its inner recording is the raw, unedited source.
 */

import { browser } from "$app/environment";

export type UploadPhase = "preparing" | "uploading" | "finalizing" | "sharing";

export interface UploadHandlers {
	workspaceId?: string;
	onPhase?: (phase: UploadPhase) => void;
	/** Byte progress 0–100 during the video PUT. */
	onProgress?: (pct: number) => void;
	share?: ShareOptions;
	/**
	 * When `false`, upload + publish only and skip minting the share link — the
	 * caller creates it afterwards via `createRecastShare`. Powers the
	 * step-by-step dialog that configures sharing *after* the upload finishes.
	 * Defaults to `true` (the quick-share behaviour home/library rely on).
	 */
	autoShare?: boolean;
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

/** Accept attribute + the guard below — keep them in sync. */
export const UPLOAD_ACCEPT = "video/mp4,video/webm,.mp4,.webm";

export function isUploadableVideo(file: File): boolean {
	return (
		file.type === "video/mp4" ||
		file.type === "video/webm" ||
		/\.(mp4|webm)$/i.test(file.name)
	);
}

/** The MIME we upload the file under — mirrored on `/init` (to sign the PUT)
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
			return "The upload didn't arrive — please try again.";
		case "empty_upload":
			return "That file came through empty — please try again.";
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

function loadVideoElement(url: string): Promise<HTMLVideoElement> {
	return new Promise((resolve, reject) => {
		const v = document.createElement("video");
		v.preload = "auto";
		v.muted = true;
		v.onloadedmetadata = () => resolve(v);
		v.onerror = () => reject(new Error("Couldn't read this video file."));
		v.src = url;
	});
}

function seekTo(video: HTMLVideoElement, time: number): Promise<void> {
	return new Promise((resolve) => {
		const done = () => {
			video.removeEventListener("seeked", done);
			resolve();
		};
		video.addEventListener("seeked", done);
		try {
			video.currentTime = time;
		} catch {
			video.removeEventListener("seeked", done);
			resolve();
		}
	});
}

/**
 * Grab a frame ~25% in and encode it as WebP (lighter than PNG/JPEG at
 * equal quality). Best-effort — returns null on any failure; the recast
 * just keeps no poster.
 */
async function capturePosterWebp(video: HTMLVideoElement): Promise<Blob | null> {
	try {
		const w = video.videoWidth;
		const h = video.videoHeight;
		if (!w || !h) return null;

		const duration = video.duration || 0;
		const target = duration > 0 ? Math.min(duration * 0.25, Math.max(0, duration - 0.1)) : 0;
		await seekTo(video, target);

		const scaleW = Math.min(960, w);
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
	} catch {
		return null;
	}
}

// ── signed PUT with progress (fetch has no upload progress) ────────────

function putWithProgress(
	envelope: SignedEnvelope,
	body: Blob,
	contentTypeFallback: string,
	onProgress?: (pct: number) => void,
): Promise<number> {
	return new Promise((resolve, reject) => {
		const xhr = new XMLHttpRequest();
		xhr.open("PUT", envelope.url);

		const headers = envelope.headers ?? {};
		const hasContentType = Object.keys(headers).some(
			(k) => k.toLowerCase() === "content-type",
		);
		for (const [k, v] of Object.entries(headers)) xhr.setRequestHeader(k, v);
		// Presigned PUTs sign the content-type; match what /init signed when the
		// envelope didn't carry it explicitly.
		if (!hasContentType) xhr.setRequestHeader("Content-Type", contentTypeFallback);

		if (onProgress) {
			xhr.upload.onprogress = (e) => {
				if (e.lengthComputable) onProgress(Math.round((e.loaded / e.total) * 100));
			};
		}
		xhr.onload = () => resolve(xhr.status);
		xhr.onerror = () => reject(new Error("Upload failed — check your connection."));
		xhr.onabort = () => reject(new Error("Upload was cancelled."));
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
	const objectUrl = URL.createObjectURL(file);
	let video: HTMLVideoElement | null = null;

	try {
		video = await loadVideoElement(objectUrl);
		const durationSec = Math.max(0, Math.round(video.duration || 0));
		const width = video.videoWidth || undefined;
		const height = video.videoHeight || undefined;
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

		// Capture the poster while the object URL is still alive (before the
		// big PUT, so a slow encode doesn't delay finalize).
		const posterBlob = await capturePosterWebp(video);

		// 2. PUT the video
		handlers.onPhase?.("uploading");
		const status = await putWithProgress(videoUpload, file, contentType, handlers.onProgress);
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
		URL.revokeObjectURL(objectUrl);
		if (video) {
			video.removeAttribute("src");
			video.load();
		}
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
		throw new Error(
			(shareData?.message as string) ?? "Couldn't create a share link.",
		);
	}
	return {
		slug: shareData.slug as string,
		shareUrl: shareData.shareUrl as string,
	};
}
