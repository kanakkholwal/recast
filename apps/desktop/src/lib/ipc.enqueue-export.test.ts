import { exportPayload } from "@recast/editor/lib/services/export-payload";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { enqueueExport } from "./ipc";

const { invoke } = vi.hoisted(() => ({
	invoke: vi.fn(async (_cmd: string, _args?: unknown) => [] as string[]),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke, Channel: class {} }));
vi.mock("$lib/analytics/client", () => ({ analytics: { capture: vi.fn() } }));

type Options = Parameters<typeof exportPayload>[0];

function payload(extra: Partial<Options> = {}) {
	return exportPayload({
		inputPath: "C:/recasts/QA.recast",
		format: "mp4",
		quality: "source",
		renderState: {} as never,
		exportId: "e1",
		...extra,
	} as Options);
}

async function send(p: ReturnType<typeof payload>): Promise<Record<string, unknown>> {
	await enqueueExport(p as Parameters<typeof enqueueExport>[0]);
	const [command, args] = invoke.mock.calls.at(-1) as [
		string,
		{ request: Record<string, unknown> },
	];
	expect(command).toBe("enqueue_export");
	return args.request;
}

/**
 * The payload builder was tested; the hop after it was not. This wrapper copied the
 * request field by field and left out `engineExport`, so every export ran on FFmpeg.
 */
describe("enqueueExport", () => {
	beforeEach(() => invoke.mockClear());

	it("carries the engine export flag through to Rust", async () => {
		const request = await send(payload({ engineExport: true }));

		expect(request.engineExport).toBe(true);
	});

	it("leaves it off when the editor did", async () => {
		const request = await send(payload());

		expect(request.engineExport).toBe(false);
	});

	it("forwards every field the payload builds, so the next new field cannot be dropped", async () => {
		const built = payload({ engineExport: true });

		const request = await send(built);

		expect(Object.keys(request).sort()).toEqual(expect.arrayContaining(Object.keys(built).sort()));
	});
});
