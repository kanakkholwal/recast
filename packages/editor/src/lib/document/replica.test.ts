import { describe, expect, it } from "vitest";
import { FakeCore, FakeDoc } from "./fakes.test-helper";
import { DocumentReplica } from "./replica";
import type { DocumentChanged, DocumentOp } from "./types";

function open(core: FakeCore) {
	return DocumentReplica.open({
		driver: core,
		path: "P.recast",
		parse: (text) => new FakeDoc(JSON.parse(text) as Record<string, string>),
	});
}

describe("DocumentReplica", () => {
	it("opens from the core's snapshot and reads the state out of it", async () => {
		const core = new FakeCore();
		const replica = await open(core);
		expect(replica.seq).toBe(0);
		expect(replica.renderState()).toEqual({ padding: 4, trimEnd: 0 });
	});

	it("a state that matches the document commits nothing", async () => {
		const core = new FakeCore();
		const replica = await open(core);
		expect(await replica.commit({ padding: 4 })).toEqual({ status: "clean" });
		expect(core.applies).toBe(0);
	});

	it("a changed state lands as one batch and advances seq on both sides", async () => {
		const core = new FakeCore();
		const replica = await open(core);
		const result = await replica.commit({ padding: 12 });
		expect(result).toEqual({ status: "applied", seq: 1, ops: 1 });
		expect(replica.seq).toBe(1);
		expect(replica.hash).toBe(core.doc.hash());
		expect(core.doc.attrs.pad).toBe("12");
		expect(core.shows).toBe(1);
	});

	it("after the first commit only the fields that moved cross into the document", async () => {
		const core = new FakeCore();
		const replica = await open(core);
		await replica.commit({ padding: 12, trimEnd: 0 });
		const spy: string[] = [];
		const inner = FakeDoc.prototype.opsForPatch;
		FakeDoc.prototype.opsForPatch = function (this: FakeDoc, patch: string) {
			spy.push(patch);
			return inner.call(this, patch);
		};
		try {
			await replica.commit({ padding: 12, trimEnd: 30 });
		} finally {
			FakeDoc.prototype.opsForPatch = inner;
		}
		expect(spy).toEqual(['{"trimEnd":30}']);
		expect(core.doc.attrs).toEqual({ pad: "12", trimEnd: "30" });
	});

	it("a stale commit merges the missed ops in and lands on top", async () => {
		const core = new FakeCore();
		const replica = await open(core);
		core.write([{ op: "set", id: "/", attr: "trimEnd", value: "30" }]);
		const result = await replica.commit({ padding: 12, trimEnd: 0 });
		expect(result).toEqual({ status: "rebased", seq: 2, ops: 1, overlaps: [] });
		expect(core.doc.attrs).toEqual({ pad: "12", trimEnd: "30" });
		expect(replica.renderState()).toEqual({ padding: 12, trimEnd: 30 });
		expect(core.shows).toBe(1);
	});

	it("a rebase names the properties both sides wrote, since ours overrode theirs", async () => {
		const core = new FakeCore();
		const replica = await open(core);
		core.write([{ op: "set", id: "/", attr: "pad", value: "9" }]);
		const result = await replica.commit({ padding: 12 });
		expect(result).toMatchObject({
			status: "rebased",
			overlaps: [{ op: "set", id: "/", attr: "pad", value: "12" }],
		});
		expect(core.doc.attrs.pad).toBe("12");
	});

	it("a commit that cannot land on the moved document reports a conflict and holds the other version", async () => {
		const core = new FakeCore();
		const replica = await open(core);
		core.write([{ op: "set", id: "/", attr: "trimEnd", value: "30" }]);
		core.failNext = null;
		const original = core.apply.bind(core);
		let calls = 0;
		core.apply = (path, ops, expectSeq) => {
			calls++;
			if (calls === 2) return Promise.reject(new Error("op 0: nothing at 'gone'"));
			return original(path, ops, expectSeq);
		};
		const result = await replica.commit({ padding: 12, trimEnd: 0 });
		expect(result).toMatchObject({ status: "conflict", seq: 1 });
		expect(replica.renderState()).toEqual({ padding: 4, trimEnd: 30 });
	});

	it("pull folds in another writer's batches and reports the state to adopt", async () => {
		const core = new FakeCore();
		const replica = await open(core);
		expect(await replica.pull()).toBeNull();
		core.write([{ op: "set", id: "/", attr: "pad", value: "9" }]);
		expect(await replica.pull()).toEqual({ padding: 9, trimEnd: 0 });
		expect(replica.seq).toBe(1);
	});

	it("a change event is news only when it is past the replica's own seq", async () => {
		const core = new FakeCore();
		const replica = await open(core);
		const seen: DocumentChanged[] = [];
		await core.subscribe((e) => seen.push(e));
		await replica.commit({ padding: 12 });
		expect(seen).toHaveLength(1);
		expect(replica.isNews(seen[0])).toBe(false);
		core.write([{ op: "set", id: "/", attr: "trimEnd", value: "60" }]);
		expect(replica.isNews(seen[1])).toBe(true);
		expect(replica.isNews({ ...seen[1], path: "other.recast" })).toBe(false);
	});

	it("a ring miss falls back to a full read", async () => {
		const core = new FakeCore();
		const replica = await open(core);
		core.write([{ op: "set", id: "/", attr: "pad", value: "9" }]);
		core.since = async () => ({
			ops: null as DocumentOp[] | null,
			seq: core.seq,
			hash: core.doc.hash(),
		});
		expect(await replica.pull()).toEqual({ padding: 9, trimEnd: 0 });
		expect(core.shows).toBe(2);
	});

	it("disposal frees the document and refuses further work", async () => {
		const core = new FakeCore();
		const replica = await open(core);
		replica.dispose();
		await expect(replica.commit({ padding: 1 })).rejects.toThrow(/disposed/);
	});
});
