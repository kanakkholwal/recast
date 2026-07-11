/**
 * Export activity store: a `$state`-backed singleton that is the single source
 * of truth for the current export job (running progress + terminal result) and
 * whether the editor's export surface is foregrounded.
 *
 * The editor page drives the actual encode (IPC) and writes lifecycle updates
 * here; the editor reads `job`/`foreground` back to decide what its export panel
 * shows, and the titlebar activity center reads `job` to surface progress and
 * completion (mirroring the cloudShare / gdrive upload stores).
 *
 * Export is bound to the editor session, so the job is cleared when the editor
 * unmounts. There is only ever one export at a time.
 */

export type ExportJobPhase =
  | "preparing"
  | "encoding"
  | "finalizing"
  | "cancelling";

export type ExportJobStatus = "running" | "success" | "error" | "cancelled";

export interface ExportJob {
  id: string;
  filename: string;
  status: ExportJobStatus;
  phase: ExportJobPhase;
  /** 0..100. Held at 100 on success. */
  progress: number;
  /** Output path once the export succeeds. */
  path?: string;
  /** Failure message once the export errors. */
  error?: string;
}

function createExportActivityStore() {
  let job = $state<ExportJob | null>(null);
  // Whether the editor's export panel is shown. Minimizing hands tracking to the
  // activity center; reopening from there (or the toolbar) sets it back.
  let foreground = $state(false);

  return {
    get job() {
      return job;
    },
    /** A job exists (running or holding a terminal result not yet dismissed). */
    get active() {
      return job !== null;
    },
    get running() {
      return job?.status === "running";
    },
    get foreground() {
      return foreground;
    },

    /** Start tracking a new export. */
    begin(id: string, filename: string) {
      job = {
        id,
        filename,
        status: "running",
        phase: "preparing",
        progress: 0,
      };
    },
    setPhase(phase: ExportJobPhase) {
      if (job && job.status === "running") job.phase = phase;
    },
    setProgress(pct: number) {
      if (!job || job.status !== "running") return;
      if (job.phase === "preparing") job.phase = "encoding";
      job.progress = Math.max(0, Math.min(100, pct));
    },
    succeed(path: string) {
      if (!job) return;
      job.status = "success";
      job.path = path;
      job.progress = 100;
    },
    fail(message: string) {
      if (!job) return;
      job.status = "error";
      job.error = message;
    },
    markCancelled() {
      if (!job) return;
      job.status = "cancelled";
    },

    /** Show the export panel in the editor. */
    show() {
      foreground = true;
    },
    /** Hide the panel and keep tracking in the activity center. */
    minimize() {
      foreground = false;
    },

    /** Drop the job entirely (dismiss a result, or leave the editor). */
    dismiss() {
      job = null;
      foreground = false;
    },
  };
}

export const exportActivity = createExportActivityStore();
