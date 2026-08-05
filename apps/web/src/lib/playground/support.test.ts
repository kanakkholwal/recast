import { describe, expect, it } from "vitest";
import { verdictFrom } from "./support";

const ok = { videoDecoder: true, videoEncoder: true, workers: true, webgl2: true, mobile: false };

describe("verdictFrom", () => {
	it("says nothing on a fully capable desktop browser", () => {
		const v = verdictFrom(ok);
		expect(v.level).toBe("full");
		expect(v.message).toBeNull();
		expect(v.canEdit).toBe(true);
	});

	// Firefox/Safari without WebCodecs: the editor cannot decode at all, so the
	// dropzone must be closed rather than failing after the file is chosen.
	it("blocks editing when a hard requirement is missing", () => {
		for (const missing of ["videoDecoder", "workers", "webgl2"] as const) {
			const v = verdictFrom({ ...ok, [missing]: false });
			expect(v.level, missing).toBe("unsupported");
			expect(v.canEdit, missing).toBe(false);
			expect(v.message, missing).toBeTruthy();
		}
	});

	// Decode without encode: editing and preview work, export doesn't. Letting
	// them edit and only failing at export would waste the whole session.
	it("allows editing but warns when encode is unavailable", () => {
		const v = verdictFrom({ ...ok, videoEncoder: false });
		expect(v.level).toBe("no-export");
		expect(v.canEdit).toBe(true);
		expect(v.message).toMatch(/export/i);
	});

	it("warns on mobile but still allows it", () => {
		const v = verdictFrom({ ...ok, mobile: true });
		expect(v.level).toBe("mobile");
		expect(v.canEdit).toBe(true);
	});

	// An unsupported engine is the more useful thing to say than "small screen".
	it("reports unsupported ahead of mobile when both apply", () => {
		expect(verdictFrom({ ...ok, mobile: true, videoDecoder: false }).level).toBe("unsupported");
	});
});
