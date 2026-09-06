import { describe, expect, it } from "vitest";
import { chooseExportEngine, type ExportEngineInputs } from "./choose-export-engine";

const READY: ExportEngineInputs = {
	masterEnabled: true,
	forceLegacy: false,
	blockedReason: null,
	capabilitySupported: true,
};

describe("chooseExportEngine", () => {
	it("routes to browser only when enabled, allowed, capable, and not forced legacy", () => {
		expect(chooseExportEngine(READY)).toEqual({ engine: "browser", reason: "browser" });
	});

	it("stays on Rust while the master switch is off", () => {
		const d = chooseExportEngine({ ...READY, masterEnabled: false });
		expect(d.engine).toBe("rust");
		expect(d.reason).toBe("browser-export-disabled");
	});

	it("honours the legacy escape hatch over capability + eligibility", () => {
		const d = chooseExportEngine({ ...READY, forceLegacy: true });
		expect(d).toEqual({ engine: "rust", reason: "user-forced-legacy" });
	});

	it("falls back to Rust when a feature gate blocks the browser path, carrying the reason", () => {
		const d = chooseExportEngine({ ...READY, blockedReason: "burned-captions" });
		expect(d).toEqual({ engine: "rust", reason: "blocked:burned-captions" });
	});

	it("falls back to Rust when the WebView can't WebCodecs-encode", () => {
		const d = chooseExportEngine({ ...READY, capabilitySupported: false });
		expect(d).toEqual({ engine: "rust", reason: "webcodecs-unsupported" });
	});

	it("disabled beats every other reason (precedence order)", () => {
		const d = chooseExportEngine({
			masterEnabled: false,
			forceLegacy: true,
			blockedReason: "x",
			capabilitySupported: false,
		});
		expect(d.reason).toBe("browser-export-disabled");
	});

	it("user-forced-legacy beats feature-block and capability", () => {
		const d = chooseExportEngine({
			masterEnabled: true,
			forceLegacy: true,
			blockedReason: "x",
			capabilitySupported: false,
		});
		expect(d.reason).toBe("user-forced-legacy");
	});
});
