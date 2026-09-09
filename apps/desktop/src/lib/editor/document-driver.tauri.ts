import type {
	DocumentApplyOutcome,
	DocumentChanged,
	DocumentDriver,
	DocumentInvalid,
	DocumentOp,
	DocumentSnapshot,
} from "@recast/editor";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** Emitted by the owner after every sequenced batch, whichever writer sent it. */
const DOCUMENT_EVENT = "document:changed";
/** Emitted by the file watcher when `project.rcx` on disk could not be taken. */
const INVALID_EVENT = "document:invalid";

/** The webview's transport onto the core's document owner (`commands::document`). */
export const tauriDocumentDriver: DocumentDriver = {
	show: (path) => invoke<DocumentSnapshot>("doc_show", { path }),

	apply: (path, ops: DocumentOp[], expectSeq) =>
		invoke<DocumentApplyOutcome>("doc_apply", { path, ops, expectSeq }),

	since: (path, seq) =>
		invoke<{ ops: DocumentOp[] | null; seq: number; hash: string }>("doc_since", { path, seq }),

	async flush(path) {
		await invoke("doc_flush", { path });
	},

	async subscribe(sink) {
		const off = await listen<DocumentChanged>(DOCUMENT_EVENT, ({ payload }) => {
			if (payload?.path) sink(payload);
		});
		return off;
	},

	async subscribeInvalid(sink) {
		const off = await listen<DocumentInvalid>(INVALID_EVENT, ({ payload }) => {
			if (payload?.path) sink(payload);
		});
		return off;
	},
};
