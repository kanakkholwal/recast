import { describe, expect, it } from "vitest";
import {
	type EndpointForm,
	emptyForm,
	formFromEndpoint,
	isValidBaseUrl,
	isValidId,
	parseLanguages,
	slugify,
	toEndpoint,
	validateForm,
} from "./remote-endpoints.logic";

describe("slugify", () => {
	it("lowercases and replaces runs of non-slug chars with a dash", () => {
		expect(slugify("LM Studio (local)")).toBe("lm-studio-local");
		expect(slugify("  My  Server!! ")).toBe("my-server");
	});
	it("keeps dots, dashes, underscores and trims leading/trailing separators", () => {
		expect(slugify(".foo.bar.")).toBe("foo.bar");
		expect(slugify("a_b-c.d")).toBe("a_b-c.d");
	});
	it("caps at 64 chars", () => {
		expect(slugify("a".repeat(100)).length).toBe(64);
	});
});

describe("isValidId", () => {
	it("accepts slugs, rejects path tricks and empties", () => {
		expect(isValidId("lmstudio-local")).toBe(true);
		expect(isValidId("acme.v2_1")).toBe(true);
		expect(isValidId("")).toBe(false);
		expect(isValidId("..")).toBe(false);
		expect(isValidId(".hidden")).toBe(false);
		expect(isValidId("has space")).toBe(false);
		expect(isValidId("a/b")).toBe(false);
	});
});

describe("isValidBaseUrl", () => {
	it("accepts absolute http(s) URLs and strips trailing slashes", () => {
		expect(isValidBaseUrl("http://127.0.0.1:1234/v1")).toBe(true);
		expect(isValidBaseUrl("https://api.example.com/")).toBe(true);
	});
	it("rejects empties, relative, and non-http schemes", () => {
		expect(isValidBaseUrl("")).toBe(false);
		expect(isValidBaseUrl("   ")).toBe(false);
		expect(isValidBaseUrl("ftp://x.com")).toBe(false);
		expect(isValidBaseUrl("/v1")).toBe(false);
		expect(isValidBaseUrl("not a url")).toBe(false);
	});
});

describe("validateForm", () => {
	const ok: EndpointForm = {
		id: "lmstudio",
		displayName: "LM Studio",
		baseUrl: "http://127.0.0.1:1234/v1",
		model: "whisper-large-v3",
		languages: "en, hi",
	};

	it("returns null for a complete valid form", () => {
		expect(validateForm(ok)).toBeNull();
	});
	it("flags each missing/invalid field", () => {
		expect(validateForm({ ...ok, displayName: " " })).toMatch(/name/i);
		expect(validateForm({ ...ok, id: "bad id" })).toMatch(/id/i);
		expect(validateForm({ ...ok, baseUrl: "nope" })).toMatch(/url/i);
		expect(validateForm({ ...ok, model: "" })).toMatch(/model/i);
	});
});

describe("parseLanguages", () => {
	it("splits on commas and whitespace, dropping empties", () => {
		expect(parseLanguages("en, hi  fr")).toEqual(["en", "hi", "fr"]);
		expect(parseLanguages("   ")).toEqual([]);
		expect(parseLanguages("")).toEqual([]);
	});
});

describe("toEndpoint / formFromEndpoint round-trip", () => {
	it("builds a payload with a normalized base URL and parsed languages", () => {
		const ep = toEndpoint({
			id: "  lmstudio ",
			displayName: "  LM Studio ",
			baseUrl: "http://127.0.0.1:1234/v1///",
			model: "  whisper  ",
			languages: "en, hi",
		});
		expect(ep).toEqual({
			id: "lmstudio",
			displayName: "LM Studio",
			baseUrl: "http://127.0.0.1:1234/v1",
			model: "whisper",
			languages: ["en", "hi"],
		});
	});
	it("prefills a form from an endpoint", () => {
		const form = formFromEndpoint({
			id: "x",
			displayName: "X",
			baseUrl: "https://x.com",
			model: "m",
			languages: ["en", "fr"],
		});
		expect(form.languages).toBe("en, fr");
		expect(form.id).toBe("x");
	});
	it("emptyForm has all blank fields", () => {
		expect(emptyForm()).toEqual({
			id: "",
			displayName: "",
			baseUrl: "",
			model: "",
			languages: "",
		});
	});
});
