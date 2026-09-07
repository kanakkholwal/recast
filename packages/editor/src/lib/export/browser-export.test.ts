import { beforeEach, describe, expect, it, vi } from "vitest";

const runExportJobInWorker = vi.fn();
const runExportJob = vi.fn();
const closeJobBitmaps = vi.fn();
const exportWorkerSupported = vi.fn(() => true);

vi.mock("./export-worker-client", () => ({
	exportWorkerSupported: () => exportWorkerSupported(),
	runExportJobInWorker: (...args: unknown[]) => runExportJobInWorker(...args),
}));
vi.mock("./run-export-job", () => ({
	runExportJob: (...args: unknown[]) => runExportJob(...args),
}));
vi.mock("./export-job", () => ({
	closeJobBitmaps: (...args: unknown[]) => closeJobBitmaps(...args),
}));
// Pulls the assets store, whose runes need the Svelte compiler; nothing here calls it.
vi.mock("../editor/services", () => ({ getEditorServices: () => ({ exportSink: null }) }));
vi.mock("./build-export-job", () => ({ buildExportJob: vi.fn() }));

const { renderJobToBytes } = await import("./browser-export");

const job = { id: "job" } as never;
const bytes = new Uint8Array([1, 2, 3]);

beforeEach(() => {
	vi.clearAllMocks();
	exportWorkerSupported.mockReturnValue(true);
	// The fallback logs a warning by design; swallow it so the run stays readable.
	vi.spyOn(console, "warn").mockImplementation(() => undefined);
});

describe("renderJobToBytes", () => {
	it("uses the worker when one is available", async () => {
		runExportJobInWorker.mockResolvedValue(bytes);
		await expect(renderJobToBytes(job, {})).resolves.toBe(bytes);
		expect(runExportJob).not.toHaveBeenCalled();
	});

	// The fallback ladder is what makes the engine safe as a default: a dead worker must not lose the export.
	it("falls back to the main thread when the worker fails", async () => {
		runExportJobInWorker.mockRejectedValue(new Error("worker died"));
		runExportJob.mockResolvedValue(bytes);
		await expect(renderJobToBytes(job, {})).resolves.toBe(bytes);
		expect(runExportJob).toHaveBeenCalledWith(job, {});
	});

	// A cancel arrives as a rejection too, and retrying would render a job the user just stopped.
	it("does not retry after a cancellation", async () => {
		const signal = AbortSignal.abort();
		runExportJobInWorker.mockRejectedValue(new Error("aborted"));
		await expect(renderJobToBytes(job, { signal })).rejects.toThrow("aborted");
		expect(runExportJob).not.toHaveBeenCalled();
		expect(closeJobBitmaps).toHaveBeenCalledWith(job);
	});

	it("renders on the main thread where no worker exists", async () => {
		exportWorkerSupported.mockReturnValue(false);
		runExportJob.mockResolvedValue(bytes);
		await expect(renderJobToBytes(job, {})).resolves.toBe(bytes);
		expect(runExportJobInWorker).not.toHaveBeenCalled();
	});

	// The worker got clones, so the originals are ours to free; leaking them held a frame set per export.
	it("frees the bitmaps it kept when the worker succeeds", async () => {
		runExportJobInWorker.mockResolvedValue(bytes);
		await renderJobToBytes(job, {});
		expect(closeJobBitmaps).toHaveBeenCalledWith(job);
	});
});
