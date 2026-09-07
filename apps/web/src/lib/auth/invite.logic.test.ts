import { describe, expect, it } from "vitest";
import {
	firstNameOf,
	INVITE_TOKEN_TTL_MS,
	inviteDisplayName,
	resetTokenIdentifier,
	setPasswordUrl,
	stripTrailingSlash,
} from "./invite.logic";

describe("resetTokenIdentifier", () => {
	it("matches the identifier Better Auth looks the token up under", () => {
		// If this prefix drifts from better-auth's password.mjs, every invite link silently 400s, so pin it.
		expect(resetTokenIdentifier("abc123")).toBe("reset-password:abc123");
	});
});

describe("setPasswordUrl", () => {
	it("points at the app's own reset page, not better-auth's redirect hop", () => {
		expect(setPasswordUrl("https://recast.li", "tok")).toBe(
			"https://recast.li/reset-password?token=tok",
		);
	});

	it("doesn't double up the slash when the origin has a trailing one", () => {
		expect(setPasswordUrl("https://recast.li/", "tok")).toBe(
			"https://recast.li/reset-password?token=tok",
		);
	});

	it("percent-encodes the token so it survives the query string", () => {
		expect(setPasswordUrl("https://recast.li", "a+b/c=")).toBe(
			"https://recast.li/reset-password?token=a%2Bb%2Fc%3D",
		);
	});
});

describe("stripTrailingSlash", () => {
	it("collapses repeated trailing slashes but leaves the path alone", () => {
		expect(stripTrailingSlash("http://localhost:5173///")).toBe("http://localhost:5173");
		expect(stripTrailingSlash("https://recast.li/app")).toBe("https://recast.li/app");
	});
});

describe("inviteDisplayName", () => {
	it("falls back to the local part when the admin leaves name blank", () => {
		expect(inviteDisplayName("", "kanak@example.com")).toBe("kanak");
		expect(inviteDisplayName("   ", "Kanak@Example.com")).toBe("kanak");
	});

	it("keeps a supplied name and caps it at the column's 80 chars", () => {
		expect(inviteDisplayName("  Kanak Kholwal ", "k@e.com")).toBe("Kanak Kholwal");
		expect(inviteDisplayName("x".repeat(200), "k@e.com")).toHaveLength(80);
	});
});

describe("firstNameOf", () => {
	it("returns null rather than an empty greeting", () => {
		// The templates render "Hi," when this is null, not "Hi ,".
		expect(firstNameOf(null)).toBe(null);
		expect(firstNameOf("   ")).toBe(null);
		expect(firstNameOf("Kanak Kholwal")).toBe("Kanak");
	});
});

describe("INVITE_TOKEN_TTL_MS", () => {
	it("outlives better-auth's 1h reset default", () => {
		// An admin approving overnight shouldn't hand out a dead link.
		expect(INVITE_TOKEN_TTL_MS).toBe(7 * 24 * 60 * 60 * 1000);
		expect(INVITE_TOKEN_TTL_MS).toBeGreaterThan(60 * 60 * 1000);
	});
});
