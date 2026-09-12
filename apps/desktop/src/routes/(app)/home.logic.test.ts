import { describe, expect, it } from "vitest";
import { greeting } from "./home.logic";

describe("greeting", () => {
	it("splits the day into morning, afternoon, evening", () => {
		expect(greeting(new Date(2026, 0, 1, 8))).toBe("Good morning");
		expect(greeting(new Date(2026, 0, 1, 13))).toBe("Good afternoon");
		expect(greeting(new Date(2026, 0, 1, 20))).toBe("Good evening");
	});

	it("treats noon as afternoon and 6pm as evening", () => {
		expect(greeting(new Date(2026, 0, 1, 12))).toBe("Good afternoon");
		expect(greeting(new Date(2026, 0, 1, 18))).toBe("Good evening");
	});
});
