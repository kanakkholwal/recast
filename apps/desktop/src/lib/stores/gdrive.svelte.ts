import { isTauriApp } from "$lib/runtime/tauri";
import {
	gdriveCancelUpload,
	gdriveConnect,
	gdriveDisconnect,
	gdriveForgetUpload,
	gdriveListUploads,
	gdriveStatus,
	gdriveUpload,
	type GdriveUploadRecord,
	type GdriveUploadResult,
} from "$lib/ipc";

/**
 * Google Drive store — a `$state`-backed singleton the UI binds to. Thin shell
 * over Tauri commands/events; the OAuth + Drive REST plumbing lives in
 * `commands/gdrive.rs`. Lazy imports keep it safe to load in the web build.
 */

export type GdriveUploadStatus = "uploading" | "complete" | "error" | "cancelled";

export type GdriveUpload = {
	uploadId: string;
	/**
	 * The local source path being uploaded. Lets list views look up
	 * "is this row currently uploading?" without scanning by filename.
	 */
	sourcePath: string;
	fileName: string;
	bytesSent: number;
	totalBytes: number;
	status: GdriveUploadStatus;
	webViewLink?: string;
	error?: string;
};

function createGdriveStore() {
	let connected = $state(false);
	let email = $state<string | null>(null);
	let connecting = $state(false);
	const uploads = $state<Record<string, GdriveUpload>>({});
	/**
	 * History of completed uploads, indexed by local file path. Hydrated
	 * from disk on `init()` via `gdrive_list_uploads`, and incrementally
	 * updated when an `upload()` call resolves. Drives the exports
	 * list dropdown ("Upload to Drive" vs. "Copy link / Re-upload").
	 */
	const uploadHistory = $state<Record<string, GdriveUploadRecord>>({});

	let listenersAttached = false;

	async function attachListeners() {
		if (listenersAttached) return;
		if (!(await isTauriApp())) return;
		listenersAttached = true;
		const { listen } = await import("@tauri-apps/api/event");

		await listen<{ connected: boolean; email?: string | null }>(
			"gdrive:connected",
			({ payload }) => {
				connected = payload.connected;
				email = payload.email ?? null;
				connecting = false;
			},
		);
		// Byte progress now streams on each upload's own channel (see `upload`),
		// and success rides the resolved `gdriveUpload` promise. Only `connected`
		// (a connection broadcast) and `upload-error` (carries the cancelled/failed
		// distinction, and backs up the corner card) stay as global events.
		await listen<{ uploadId: string; message: string; cancelled: boolean }>(
			"gdrive:upload-error",
			({ payload }) => {
				const existing = uploads[payload.uploadId];
				if (!existing) return;
				uploads[payload.uploadId] = {
					...existing,
					status: payload.cancelled ? "cancelled" : "error",
					error: payload.cancelled ? undefined : payload.message,
				};
			},
		);
	}

	/** Read current connection state from the Rust side. Best-effort. */
	async function refreshStatus() {
		if (!(await isTauriApp())) return;
		try {
			const status = await gdriveStatus();
			connected = status.connected;
			email = status.email ?? null;
		} catch (e) {
			console.error("[gdrive] status check failed", e);
		}
	}

	/** Pull the upload history from disk into the in-memory map. */
	async function refreshHistory() {
		if (!(await isTauriApp())) return;
		try {
			const records = await gdriveListUploads();
			// Wipe then refill so deletions elsewhere propagate.
			for (const key of Object.keys(uploadHistory)) {
				delete uploadHistory[key];
			}
			for (const [path, record] of Object.entries(records ?? {})) {
				uploadHistory[path] = record;
			}
		} catch (e) {
			console.error("[gdrive] history load failed", e);
		}
	}

	/**
	 * Start the OAuth flow. The Rust side handles the browser/callback/token
	 * exchange and emits `gdrive:connected`; we just flip `connecting`.
	 */
	async function connect() {
		if (!(await isTauriApp())) return;
		await attachListeners();
		connecting = true;
		try {
			await gdriveConnect();
			// Success: the `gdrive:connected` listener flips state.
		} catch (e) {
			connecting = false;
			console.error("[gdrive] connect failed", e);
			throw e;
		}
	}

	async function disconnect() {
		if (!(await isTauriApp())) return;
		try {
			await gdriveDisconnect();
		} catch (e) {
			console.error("[gdrive] disconnect failed", e);
		}
		connected = false;
		email = null;
	}

	/**
	 * Kick off an upload. Resolves with the result or rejects on failure, but
	 * the corner-card UI usually relies on the `uploads` map updating via
	 * events rather than awaiting this.
	 */
	async function upload(path: string): Promise<GdriveUploadResult> {
		if (!(await isTauriApp())) throw new Error("not running in Tauri");
		await attachListeners();
		const fileName = path.split(/[\\/]/).pop() ?? path;
		const uploadId = `upload-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
		uploads[uploadId] = {
			uploadId,
			sourcePath: path,
			fileName,
			bytesSent: 0,
			totalBytes: 0,
			status: "uploading",
		};
		try {
			// Byte progress rides this upload's own channel.
			const result = await gdriveUpload(path, uploadId, (p) => {
				const existing = uploads[uploadId];
				if (!existing) return;
				uploads[uploadId] = {
					...existing,
					bytesSent: p.bytesSent,
					totalBytes: p.totalBytes,
				};
			});
			// Success is the resolved result (the data the old `upload-complete`
			// event carried), so update the card + history here.
			const existing = uploads[uploadId];
			if (existing) {
				uploads[uploadId] = {
					...existing,
					status: "complete",
					bytesSent: existing.totalBytes || existing.bytesSent,
					webViewLink: result.webViewLink,
				};
			}
			uploadHistory[path] = {
				fileId: result.fileId,
				name: result.name,
				webViewLink: result.webViewLink,
				uploadedAt: Math.floor(Date.now() / 1000),
			};
			return result;
		} catch (e) {
			// Rust also fires a detached `gdrive:upload-error` (with the
			// cancelled/failed flag) for the card; re-throw for awaiting callers.
			throw e;
		}
	}

	async function cancelUpload(uploadId: string) {
		if (!(await isTauriApp())) return;
		try {
			await gdriveCancelUpload(uploadId);
		} catch (e) {
			console.error("[gdrive] cancel failed", e);
		}
	}

	function dismissUpload(uploadId: string) {
		delete uploads[uploadId];
	}

	/** Drop a path from upload history (e.g. local file deleted). The Drive
	 *  file itself isn't touched. */
	async function forgetUpload(localPath: string) {
		delete uploadHistory[localPath];
		if (!(await isTauriApp())) return;
		try {
			await gdriveForgetUpload(localPath);
		} catch (e) {
			console.error("[gdrive] forget failed", e);
		}
	}

	/** Look up the persisted record for a local export, if any. */
	function getRecordForPath(localPath: string): GdriveUploadRecord | undefined {
		return uploadHistory[localPath];
	}

	/** In-flight upload for a source path, most-recent first if several match. */
	function getActiveUploadForPath(
		localPath: string,
	): GdriveUpload | undefined {
		const list = Object.values(uploads).filter(
			(u) => u.sourcePath === localPath && u.status === "uploading",
		);
		// uploadIds are timestamp-prefixed, so lexicographic max = most recent.
		list.sort((a, b) => b.uploadId.localeCompare(a.uploadId));
		return list[0];
	}

	return {
		get connected() {
			return connected;
		},
		get email() {
			return email;
		},
		get connecting() {
			return connecting;
		},
		get uploads() {
			return uploads;
		},
		get activeUploads() {
			return Object.values(uploads);
		},
		get uploadHistory() {
			return uploadHistory;
		},

		/** Wire event listeners and pull current status + history. Safe to call repeatedly. */
		async init() {
			await attachListeners();
			await refreshStatus();
			await refreshHistory();
		},

		refreshStatus,
		refreshHistory,
		connect,
		disconnect,
		upload,
		cancelUpload,
		dismissUpload,
		forgetUpload,
		getRecordForPath,
		getActiveUploadForPath,
	};
}

export const gdrive = createGdriveStore();
