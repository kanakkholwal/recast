import { describe, expect, it } from "vitest";
import type { DeepLinkAction } from "./deepLink.logic";
import {
	buildDeepLink,
	buildNavigateLink,
	buildOpenProjectLink,
	parseDeepLink,
} from "./deepLink.logic";

describe("parseDeepLink", () => {
	it("parses open?path into an open-project action", () => {
		expect(parseDeepLink("recast://open?path=%2Ftmp%2Fdemo.recast")).toEqual({
			kind: "open-project",
			path: "/tmp/demo.recast",
		});
	});

	it("round-trips a uri-encoded Windows path", () => {
		const path = "C:\\Users\\kanak\\Recordings\\demo.recast";
		const url = `recast://open?path=${encodeURIComponent(path)}`;
		expect(parseDeepLink(url)).toEqual({ kind: "open-project", path });
	});

	it("parses go?to into a navigate action for each allowlisted route", () => {
		for (const route of ["/", "/recasts", "/exports", "/profiles", "/settings", "/whats-new"]) {
			expect(parseDeepLink(`recast://go?to=${encodeURIComponent(route)}`)).toEqual({
				kind: "navigate",
				route,
			});
		}
	});

	it("allows deeper subpaths under an allowlisted segment", () => {
		expect(parseDeepLink("recast://go?to=%2Fsettings%2Fcloud")).toEqual({
			kind: "navigate",
			route: "/settings/cloud",
		});
	});

	it("rejects a route outside the allowlist", () => {
		expect(parseDeepLink("recast://go?to=%2Fsecret")).toBeNull();
	});

	it("rejects protocol-relative and traversal routes", () => {
		expect(parseDeepLink("recast://go?to=%2F%2Fevil.com")).toBeNull();
		expect(parseDeepLink("recast://go?to=%2F..%2Fetc")).toBeNull();
		expect(parseDeepLink("recast://go?to=exports")).toBeNull();
	});

	it("rejects the wrong protocol", () => {
		expect(parseDeepLink("https://open?path=%2Ftmp%2Fx.recast")).toBeNull();
	});

	it("rejects an unknown host", () => {
		expect(parseDeepLink("recast://bogus?x=1")).toBeNull();
	});

	it("rejects malformed input and missing query params", () => {
		expect(parseDeepLink("recast://")).toBeNull();
		expect(parseDeepLink("not a url")).toBeNull();
		expect(parseDeepLink("recast://open")).toBeNull();
		expect(parseDeepLink("recast://go")).toBeNull();
	});
});

describe("deep-link builders", () => {
	it("builds an open-project link", () => {
		expect(buildOpenProjectLink("/tmp/demo.recast")).toBe(
			"recast://open?path=%2Ftmp%2Fdemo.recast",
		);
	});

	it("builds a navigate link for an allowlisted route", () => {
		expect(buildNavigateLink("/settings/cloud")).toBe("recast://go?to=%2Fsettings%2Fcloud");
	});

	it("throws when building a navigate link for a disallowed route", () => {
		expect(() => buildNavigateLink("/secret")).toThrow();
		expect(() => buildNavigateLink("//evil.com")).toThrow();
		expect(() => buildNavigateLink("exports")).toThrow();
	});

	it("round-trips every action shape through build → parse", () => {
		const actions: DeepLinkAction[] = [
			{ kind: "open-project", path: "C:\\Users\\kanak\\demo.recast" },
			{ kind: "open-project", path: "/tmp/a b/clip.recast" },
			{ kind: "navigate", route: "/" },
			{ kind: "navigate", route: "/exports" },
			{ kind: "navigate", route: "/settings/cloud" },
		];
		for (const action of actions) {
			expect(parseDeepLink(buildDeepLink(action))).toEqual(action);
		}
	});
});
