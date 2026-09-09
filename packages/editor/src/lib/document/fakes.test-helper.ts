import type {
	DocumentApplyOutcome,
	DocumentChanged,
	DocumentDriver,
	DocumentInvalid,
	DocumentOp,
	ReplicaDocument,
} from "./types";

/** A one-element document: root attributes only. Enough to exercise the protocol, not the mapping. */
export class FakeDoc implements ReplicaDocument {
	constructor(public attrs: Record<string, string>) {}
	free() {
		this.attrs = {};
	}
	text() {
		return JSON.stringify(this.attrs);
	}
	hash() {
		return `h:${JSON.stringify(Object.entries(this.attrs).sort())}`;
	}
	apply(opsJson: string) {
		const ops = JSON.parse(opsJson) as DocumentOp[];
		const next = { ...this.attrs };
		for (const op of ops) {
			if (op.op !== "set" || op.id !== "/") throw `unsupported op ${op.op}`;
			if (op.value === null) delete next[op.attr];
			else next[op.attr] = op.value;
		}
		this.attrs = next;
		return ops.length;
	}
	renderState() {
		return JSON.stringify({
			padding: Number(this.attrs.pad ?? 0),
			trimEnd: Number(this.attrs.trimEnd ?? 0),
		});
	}
	opsForState(stateJson: string) {
		const state = JSON.parse(stateJson) as { padding?: number; trimEnd?: number };
		const ops: DocumentOp[] = [];
		for (const [attr, value] of [
			["pad", state.padding],
			["trimEnd", state.trimEnd],
		] as const) {
			if (value !== undefined && String(value) !== (this.attrs[attr] ?? "0")) {
				ops.push({ op: "set", id: "/", attr, value: String(value) });
			}
		}
		return JSON.stringify(ops);
	}
}

/** The core: a sequencer over the same fake document, with a ring of applied batches. */
export class FakeCore implements DocumentDriver {
	doc = new FakeDoc({ pad: "4" });
	seq = 0;
	log: { seq: number; ops: DocumentOp[] }[] = [];
	sinks: ((e: DocumentChanged) => void)[] = [];
	invalidSinks: ((e: DocumentInvalid) => void)[] = [];
	applies = 0;
	shows = 0;
	flushes = 0;
	/** Set to make the next apply fail outright (an op that cannot land). */
	failNext: string | null = null;

	show() {
		this.shows++;
		return Promise.resolve({ text: this.doc.text(), hash: this.doc.hash(), seq: this.seq });
	}

	apply(_path: string, ops: DocumentOp[], expectSeq: number): Promise<DocumentApplyOutcome> {
		this.applies++;
		if (this.failNext) {
			const reason = this.failNext;
			this.failNext = null;
			return Promise.reject(new Error(reason));
		}
		if (expectSeq !== this.seq) {
			const since = this.log.filter((e) => e.seq > expectSeq).flatMap((e) => e.ops);
			return Promise.resolve({ result: "stale", seq: this.seq, hash: this.doc.hash(), since });
		}
		return Promise.resolve(this.write(ops));
	}

	/** Another writer (an agent, the CLI) landing a batch. */
	write(ops: DocumentOp[]): DocumentApplyOutcome {
		this.doc.apply(JSON.stringify(ops));
		this.seq += 1;
		this.log.push({ seq: this.seq, ops });
		for (const sink of this.sinks) sink({ path: "P.recast", seq: this.seq, hash: this.doc.hash() });
		return { result: "applied", seq: this.seq, hash: this.doc.hash() };
	}

	since(
		_path: string,
		seq: number,
	): Promise<{ ops: DocumentOp[] | null; seq: number; hash: string }> {
		const ops = this.log.filter((e) => e.seq > seq).flatMap((e) => e.ops);
		return Promise.resolve({ ops, seq: this.seq, hash: this.doc.hash() });
	}

	flush() {
		this.flushes++;
		return Promise.resolve();
	}

	/** The watcher refusing a hand edit of the file. */
	invalid(event: DocumentInvalid) {
		for (const sink of this.invalidSinks) sink(event);
	}

	subscribeInvalid(sink: (e: DocumentInvalid) => void) {
		this.invalidSinks.push(sink);
		return Promise.resolve(() => {
			this.invalidSinks = this.invalidSinks.filter((s) => s !== sink);
		});
	}

	subscribe(sink: (e: DocumentChanged) => void) {
		this.sinks.push(sink);
		return Promise.resolve(() => {
			this.sinks = this.sinks.filter((s) => s !== sink);
		});
	}
}
