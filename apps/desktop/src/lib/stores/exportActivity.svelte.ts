import { runExport, type RunExportOptions } from "$lib/services/export";
import { notifyJobDone } from "$lib/notify";
import { cancelExport, refreshTray, type ExportStateEvent } from "$lib/ipc";
import {
  clearJobProgress,
  setJobProgress,
  setJobProgressIndeterminate,
} from "$lib/taskbarProgress";
import { toast } from "@recast/ui/sonner";

/**
 * Export activity store: a `$state`-backed singleton that owns the export QUEUE
 * and runs it. It is the single source of truth for every export's progress and
 * result. The editor builds a self-contained item (render state captured at
 * enqueue) and hands it off here; this store runs items one at a time (no two
 * FFmpegs fighting for CPU/GPU), so an export survives closing its editor.
 *
 * The editor panel reads back the item it enqueued (ring / "Queued"), and the
 * titlebar activity center lists the whole queue. Mirrors the cloudShare /
 * gdrive upload stores.
 */

export type ExportItemStatus =
  | "queued"
  | "running"
  | "success"
  | "error"
  | "cancelled";

export type ExportItemPhase =
  | "preparing"
  | "encoding"
  | "finalizing"
  | "cancelling";

/** Everything needed to run an export, captured at enqueue so it can run after
 *  the source editor is closed. */
export type ExportRunParams = Omit<RunExportOptions, "exportId" | "onState">;

export interface ExportItem {
  id: string;
  filename: string;
  /** Source project path, for display + same-project panel adoption. */
  filePath: string;
  status: ExportItemStatus;
  phase: ExportItemPhase;
  /** 0..100. Held at 100 on success. */
  progress: number;
  /** When the FFmpeg run started (null while queued), for the ETA readout. */
  startedAt: number | null;
  /** Output path once it succeeds. */
  path?: string;
  /** Failure message once it errors. */
  error?: string;
  params: ExportRunParams;
}

function messageOf(e: unknown): string {
  return typeof e === "string"
    ? e
    : e instanceof Error
      ? e.message
      : String(e);
}

function baseName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

function createExportActivityStore() {
  // Running + queued + terminal-not-yet-dismissed items, in enqueue order.
  const items = $state<ExportItem[]>([]);
  // Whether the editor's export panel is shown. Minimizing hands tracking to the
  // activity center; reopening from there (or the toolbar) sets it back.
  let foreground = $state(false);
  // The item id the editor panel is currently showing (null during the options
  // picker), so the activity center can hide just that one to avoid doubling.
  let foregroundId = $state<string | null>(null);
  // Whether an editor (which hosts the export panel) is mounted, so the activity
  // center knows a "foregrounded" job actually has a panel on screen.
  let editorPresent = $state(false);

  const find = (id: string) => items.find((i) => i.id === id);
  const runningItem = () => items.find((i) => i.status === "running");

  // Progress/phase updates from the Rust pipeline. Terminal outcomes come from
  // the runExport promise below, so those events are ignored here.
  function applyState(id: string, e: ExportStateEvent) {
    const it = find(id);
    if (!it || it.status !== "running") return;
    switch (e.status) {
      case "started":
      case "preparing":
        it.phase = "preparing";
        break;
      case "progress": {
        const next = Math.min(100, Math.max(0, e.progress));
        if (it.phase === "preparing") it.phase = "encoding";
        it.progress = Math.max(it.progress, next);
        void setJobProgress(it.progress);
        break;
      }
      case "finalizing":
        it.phase = "finalizing";
        void setJobProgressIndeterminate();
        break;
    }
  }

  async function runItem(it: ExportItem) {
    it.status = "running";
    it.phase = "preparing";
    it.progress = 0;
    it.startedAt = Date.now();
    void setJobProgressIndeterminate();
    try {
      const path = await runExport({
        ...it.params,
        exportId: it.id,
        onState: (e) => applyState(it.id, e),
      });
      const cur = find(it.id);
      if (cur) {
        cur.status = "success";
        cur.path = path;
        cur.progress = 100;
      }
      toast.success("Export complete", { description: it.filename });
      void notifyJobDone("Export complete", baseName(path));
      void refreshTray(null).catch(() => {});
    } catch (e) {
      const msg = messageOf(e);
      const cur = find(it.id);
      if (cur) {
        if (cur.phase === "cancelling" || /cancel/i.test(msg)) {
          cur.status = "cancelled";
          toast.info("Export cancelled");
        } else {
          cur.status = "error";
          cur.error = msg;
          toast.error("Export failed");
        }
      }
    } finally {
      void clearJobProgress();
      processQueue();
    }
  }

  // One export at a time: start the next queued item only when nothing runs.
  function processQueue() {
    if (runningItem()) return;
    const next = items.find((i) => i.status === "queued");
    if (next) void runItem(next);
  }

  return {
    get items() {
      return items;
    },
    /** Any export currently encoding. */
    get running() {
      return runningItem() != null;
    },
    /** Any item at all (queued, running, or an undismissed result). */
    get active() {
      return items.length > 0;
    },
    get foreground() {
      return foreground;
    },
    get foregroundId() {
      return foregroundId;
    },
    get editorPresent() {
      return editorPresent;
    },

    item(id: string): ExportItem | null {
      return find(id) ?? null;
    },
    /** 1-based position of a queued item behind the running one (0 if running). */
    queuePosition(id: string): number {
      const active = items.filter(
        (i) => i.status === "queued" || i.status === "running",
      );
      return Math.max(0, active.findIndex((i) => i.id === id));
    },

    /** Editor mount lifecycle sets this (see the activity center hide rule). */
    setEditorPresent(present: boolean) {
      editorPresent = present;
    },
    /** Show the export panel in the editor for a given item (null = options). */
    show(id: string | null = null) {
      foreground = true;
      foregroundId = id;
    },
    /** Hide the panel and keep tracking in the activity center. */
    minimize() {
      foreground = false;
    },

    /** Add a fully-built export to the queue; starts immediately if idle. */
    enqueue(spec: {
      id: string;
      filename: string;
      filePath: string;
      params: ExportRunParams;
    }) {
      items.push({
        id: spec.id,
        filename: spec.filename,
        filePath: spec.filePath,
        status: "queued",
        phase: "preparing",
        progress: 0,
        startedAt: null,
        params: spec.params,
      });
      processQueue();
    },

    /** Cancel/remove an item: a queued one is dropped; a running one is stopped
     *  (the runExport promise then rejects and flips it to cancelled). */
    async cancel(id: string) {
      const it = find(id);
      if (!it) return;
      if (it.status === "queued") {
        const idx = items.indexOf(it);
        if (idx >= 0) items.splice(idx, 1);
        return;
      }
      if (it.status === "running") {
        it.phase = "cancelling";
        try {
          await cancelExport(id);
        } catch (e) {
          console.warn("[exportActivity] cancel failed", e);
        }
      }
    },

    /** Remove a finished (non-running) item from the list. */
    dismiss(id: string) {
      const idx = items.findIndex((i) => i.id === id);
      if (idx >= 0 && items[idx].status !== "running") items.splice(idx, 1);
    },
  };
}

export const exportActivity = createExportActivityStore();
