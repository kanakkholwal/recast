import { afterEach, describe, expect, it, vi } from "vitest";
import { EXPORT_IGNORE_ATTR, defaultFilename, exportFilter } from "./export";

/**
 * `exportFilter` is the only thing keeping editing chrome — rulers, grid,
 * selection handles — out of the exported image. A regression here is invisible
 * until it ships in someone's screenshot, so it gets a test even though the
 * surrounding module is DOM-bound.
 */
describe("exportFilter", () => {
	class FakeElement {
		#attrs: Set<string>;
		constructor(...attrs: string[]) {
			this.#attrs = new Set(attrs);
		}
		hasAttribute(name: string): boolean {
			return this.#attrs.has(name);
		}
	}

	afterEach(() => vi.unstubAllGlobals());

	function withElement<T>(fn: () => T): T {
		vi.stubGlobal("Element", FakeElement);
		return fn();
	}

	it("drops nodes marked with the ignore attribute", () => {
		withElement(() => {
			expect(exportFilter(new FakeElement(EXPORT_IGNORE_ATTR) as unknown as Node)).toBe(false);
		});
	});

	it("keeps ordinary elements", () => {
		withElement(() => {
			expect(exportFilter(new FakeElement("class") as unknown as Node)).toBe(true);
		});
	});

	it("keeps non-elements (text nodes carry no attributes)", () => {
		withElement(() => {
			expect(exportFilter({ nodeType: 3 } as unknown as Node)).toBe(true);
		});
	});
});

describe("defaultFilename", () => {
	it("uses the conventional .jpg for jpeg", () => {
		expect(defaultFilename("jpeg")).toBe("screenshot.jpg");
		expect(defaultFilename("png")).toBe("screenshot.png");
	});
});
