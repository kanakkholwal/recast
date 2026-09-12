import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { EditorRenderState } from "../editor/render-state";
import { FakeCore, FakeDoc } from "./fakes.test-helper";
import { DocumentSession, type SessionStore } from "./session";

/** The store as the session sees it: a state, a dirty flag, and an undo count. */
class FakeStore implements SessionStore {
	state: Partial<EditorRenderState> = { padding: 4, trimEnd: 0 } as Partial<EditorRenderState>;
	isDirty = false;
	undos = 0;
	savedAt: number[] = [];
	edit(patch: Partial<EditorRenderState>) {
		this.state = { ...this.state, ...patch };
		this.isDirty = true;
	}
	toRenderState() {
		return this.state;
	}
	loadRenderState(state: Partial<EditorRenderState>) {
		this.state = { ...state };
	}
	pushUndoState() {
		this.undos++;
	}
	markSaved(at: number) {
		this.isDirty = false;
		this.savedAt.push(at);
	}
}

async function open(
	core: FakeCore,
	store: FakeStore,
	extra: Partial<Parameters<typeof DocumentSession.open>[0]> = {},
) {
	const session = await DocumentSession.open({
		store,
		projectPath: "P.recast",
		parse: (text) => new FakeDoc(JSON.parse(text) as Record<string, string>),
		driver: core,
		now: () => 1000,
		...extra,
	});
	if (!session) throw new Error("driver was installed");
	return session;
}

describe("DocumentSession", () => {
	beforeEach(() => vi.useFakeTimers());
	afterEach(() => vi.useRealTimers());

	it("is absent without a driver, so a host without a core keeps its store alone", async () => {
		const session = await DocumentSession.open({
			store: new FakeStore(),
			projectPath: "P.recast",
			parse: () => new FakeDoc({}),
		});
		expect(session).toBeNull();
	});

	it("a dirty store lands as one batch after the mirror delay and reads as saved", async () => {
		const core = new FakeCore();
		const store = new FakeStore();
		const session = await open(core, store, { mirrorDelayMs: 100 });
		store.edit({ padding: 12 });
		session.scheduleCommit();
		store.edit({ trimEnd: 30 });
		session.scheduleCommit();
		await vi.advanceTimersByTimeAsync(99);
		expect(core.applies).toBe(0);
		await vi.advanceTimersByTimeAsync(1);
		expect(core.applies).toBe(1);
		expect(core.doc.attrs).toEqual({ pad: "12", trimEnd: "30" });
		expect(store.isDirty).toBe(false);
		expect(store.savedAt).toEqual([1000]);
	});

	it("an edit made while a commit is in flight is mirrored by the follow-up pass", async () => {
		const core = new FakeCore();
		const store = new FakeStore();
		const session = await open(core, store, { mirrorDelayMs: 100 });
		const original = core.apply.bind(core);
		core.apply = (p, ops, seq) => {
			store.edit({ trimEnd: 30 });
			core.apply = original;
			return original(p, ops, seq);
		};
		store.edit({ padding: 12 });
		await session.commit();
		expect(core.doc.attrs).toEqual({ pad: "12" });
		expect(store.isDirty).toBe(false);
		await vi.advanceTimersByTimeAsync(100);
		expect(core.doc.attrs).toEqual({ pad: "12", trimEnd: "30" });
		expect(core.applies).toBe(2);
		await vi.advanceTimersByTimeAsync(100);
		expect(core.applies).toBe(2);
	});

	it("save commits and checkpoints; a clean save still checkpoints", async () => {
		const core = new FakeCore();
		const store = new FakeStore();
		const session = await open(core, store);
		expect(await session.save()).toEqual({ status: "clean" });
		expect(core.flushes).toBe(1);
		store.edit({ padding: 7 });
		expect(await session.save()).toMatchObject({ status: "applied", seq: 1 });
		expect(core.flushes).toBe(2);
	});

	it("another writer's batch is adopted into a clean store as one undo step", async () => {
		const core = new FakeCore();
		const store = new FakeStore();
		await open(core, store);
		core.write([{ op: "set", id: "/", attr: "pad", value: "9" }]);
		await vi.advanceTimersByTimeAsync(0);
		expect(store.state).toEqual({ padding: 9, trimEnd: 0 });
		expect(store.undos).toBe(1);
		expect(store.isDirty).toBe(false);
	});

	it("another writer's batch under unsaved edits is merged, not chosen between", async () => {
		const core = new FakeCore();
		const store = new FakeStore();
		await open(core, store);
		store.edit({ padding: 12 });
		core.write([{ op: "set", id: "/", attr: "trimEnd", value: "30" }]);
		await vi.advanceTimersByTimeAsync(0);
		expect(core.doc.attrs).toEqual({ pad: "12", trimEnd: "30" });
		expect(store.state).toEqual({ padding: 12, trimEnd: 30 });
		expect(store.isDirty).toBe(false);
	});

	it("an overlapping rebase tells the host which properties the other writer lost", async () => {
		const core = new FakeCore();
		const store = new FakeStore();
		const overlaps: string[] = [];
		await open(core, store, {
			onOverlap: (ops) => overlaps.push(...ops.map((o) => (o.op === "set" ? o.attr : o.op))),
		});
		store.edit({ padding: 12 });
		core.write([{ op: "set", id: "/", attr: "pad", value: "9" }]);
		await vi.advanceTimersByTimeAsync(0);
		expect(overlaps).toEqual(["pad"]);
		expect(store.state.padding).toBe(12);
	});

	it("an invalid file on disk reaches the host only for the bound project", async () => {
		const core = new FakeCore();
		const store = new FakeStore();
		const seen: string[] = [];
		await open(core, store, { onInvalid: (e) => seen.push(e.message) });
		core.invalid({ path: "other.recast", message: "no" });
		core.invalid({
			path: "P.recast",
			message: "line 3, column 9: unexpected end",
			at: { line: 3, column: 9 },
		});
		expect(seen).toEqual(["line 3, column 9: unexpected end"]);
	});

	it("a conflict adopts the other writer's version and tells the host", async () => {
		const core = new FakeCore();
		const store = new FakeStore();
		const conflicts: string[] = [];
		await open(core, store, { onConflict: (r) => conflicts.push(r) });
		const original = core.apply.bind(core);
		let calls = 0;
		core.apply = (p, ops, seq) => {
			calls++;
			if (calls === 2) return Promise.reject(new Error("op 0: nothing at 'gone'"));
			return original(p, ops, seq);
		};
		store.edit({ padding: 12 });
		core.write([{ op: "set", id: "/", attr: "trimEnd", value: "30" }]);
		await vi.advanceTimersByTimeAsync(0);
		expect(calls).toBe(2);
		expect(conflicts).toHaveLength(1);
		expect(store.state).toEqual({ padding: 4, trimEnd: 30 });
		expect(store.isDirty).toBe(false);
	});

	it("concurrent commits share one flight", async () => {
		const core = new FakeCore();
		const store = new FakeStore();
		const session = await open(core, store);
		store.edit({ padding: 12 });
		const [a, b] = await Promise.all([session.commit(), session.commit()]);
		expect(a).toBe(b);
		expect(core.applies).toBe(1);
	});

	it("dispose mirrors what is dirty, checkpoints, and stops listening", async () => {
		const core = new FakeCore();
		const store = new FakeStore();
		const session = await open(core, store);
		store.edit({ padding: 12 });
		await session.dispose();
		expect(core.doc.attrs.pad).toBe("12");
		expect(core.flushes).toBe(1);
		expect(core.sinks).toHaveLength(0);
		core.write([{ op: "set", id: "/", attr: "pad", value: "1" }]);
		await vi.advanceTimersByTimeAsync(0);
		expect(store.state.padding).toBe(12);
	});
});
