/** Messages between the main thread and the export render worker (Phase 3). */

import type { ExportJob } from "./export-job";

export type ToExportWorker = { type: "render"; job: ExportJob } | { type: "cancel" };

export type FromExportWorker =
	| { type: "progress"; fraction: number }
	| { type: "done"; bytes: Uint8Array }
	| { type: "error"; message: string };
