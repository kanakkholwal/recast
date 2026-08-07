import { beforeEach, describe, expect, it } from "vitest";
import { setEditorHostHooks } from "../host-hooks";
import { RenderWorkerClient } from "./render-worker-client";

class FakeWorker {
	posted: Array<{ type: string }> = [];
	onmessage: ((e: { data: unknown }) => void) | null = null;
	postMessage(msg: { type: string }): void {
		this.posted.push(msg);
	}
	terminate(): void {}
	of(type: string) {
		return this.posted.filter((m) => m.type === type);
	}
}

/** `new VideoFrame(...)` doesn't exist in node; only `close()` matters here. */
function fakeFrame() {
	return {
		closed: false,
		close(this: { closed: boolean }) {
			this.closed = true;
		},
	};
}

const fakeCanvas = () =>
	({ getContext: () => ({ transferFromImageBitmap() {} }) }) as unknown as HTMLCanvasElement;

let worker: FakeWorker;

function newClient(): RenderWorkerClient {
	worker = new FakeWorker();
	setEditorHostHooks({ workers: { create: () => worker as unknown as Worker } });
	const client = new RenderWorkerClient({ canvas: fakeCanvas(), ringCapacity: 4 });
	// The worker ignores work until it reports ready; the tests below are about
	// what happens after that.
	worker.onmessage?.({ data: { type: "ready" } });
	worker.posted.length = 0;
	return client;
}

const params = { uniforms: {} as never, svgCursor: null, bindBackgroundImage: false };
const render = (client: RenderWorkerClient) =>
	client.renderFrame(params, 100, 100, 0, 0, true, true);
const ack = () => worker.onmessage?.({ data: { type: "skipped", seq: 0 } });

describe("fallback frame backpressure", () => {
	beforeEach(() => {
		newClient();
	});

	it("drops a fallback frame before the worker is ready", () => {
		worker = new FakeWorker();
		setEditorHostHooks({ workers: { create: () => worker as unknown as Worker } });
		const client = new RenderWorkerClient({ canvas: fakeCanvas(), ringCapacity: 4 });
		const frame = fakeFrame();
		client.putFallbackFrame(frame as never, 0);

		// Pre-init the worker has no GL context, so it closes whatever arrives.
		expect(worker.of("fallbackFrame")).toHaveLength(0);
		expect(frame.closed).toBe(true);
	});

	it("posts a fallback frame when the worker is idle", () => {
		const client = newClient();
		const frame = fakeFrame();
		client.putFallbackFrame(frame as never, 0);

		expect(worker.of("fallbackFrame")).toHaveLength(1);
		expect(frame.closed).toBe(false);
	});

	it("drops and closes a fallback frame while a render is in flight", () => {
		const client = newClient();
		render(client);
		const frame = fakeFrame();
		client.putFallbackFrame(frame as never, 1000);

		// The matching render would be coalesced away, so queueing the surface just
		// grows a backlog the worker never composites.
		expect(worker.of("fallbackFrame")).toHaveLength(0);
		expect(frame.closed).toBe(true);
	});

	it("resumes posting once the worker acks", () => {
		const client = newClient();
		render(client);
		client.putFallbackFrame(fakeFrame() as never, 1000);
		ack();

		const frame = fakeFrame();
		client.putFallbackFrame(frame as never, 2000);
		expect(worker.of("fallbackFrame")).toHaveLength(1);
		expect(frame.closed).toBe(false);
	});

	it("never leaks a frame: every one is either posted or closed", () => {
		const client = newClient();
		const frames = Array.from({ length: 20 }, fakeFrame);
		for (const [i, frame] of frames.entries()) {
			client.putFallbackFrame(frame as never, i * 1000);
			render(client);
		}
		const posted = worker.of("fallbackFrame").length;
		const closed = frames.filter((f) => f.closed).length;
		expect(posted + closed).toBe(frames.length);
	});
});
