import type {
	DocumentApplyOutcome,
	DocumentChanged,
	DocumentDriver,
	DocumentOp,
	DocumentSnapshot,
} from "@recast/editor";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** Emitted by `control::doc::apply` after every sequenced batch, whichever writer sent it. */
const DOCUMENT_EVENT = "document:changed";

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
};
