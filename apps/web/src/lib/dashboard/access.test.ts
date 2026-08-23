import { describe, expect, it } from "vitest";
import { canAccessRecast, isWorkspaceManager } from "./access";

const base = { recastOwnerId: "u_creator", userId: "u_other" };

describe("canAccessRecast", () => {
	it("lets the creator in whatever their workspace role", () => {
		expect(canAccessRecast({ recastOwnerId: "u_1", userId: "u_1", workspaceRole: "member" })).toBe(
			true,
		);
	});

	it("lets workspace owners and admins in", () => {
		expect(canAccessRecast({ ...base, workspaceRole: "owner" })).toBe(true);
		expect(canAccessRecast({ ...base, workspaceRole: "admin" })).toBe(true);
	});

	it("lets a platform admin in", () => {
		expect(canAccessRecast({ ...base, workspaceRole: "member", platformRole: "admin" })).toBe(true);
	});

	it("keeps another member out of someone else's recast", () => {
		expect(canAccessRecast({ ...base, workspaceRole: "member", platformRole: "user" })).toBe(false);
	});

	it("keeps out when the role is missing entirely", () => {
		expect(canAccessRecast(base)).toBe(false);
	});
});

describe("isWorkspaceManager", () => {
	it("is owner and admin only", () => {
		expect(isWorkspaceManager("owner")).toBe(true);
		expect(isWorkspaceManager("admin")).toBe(true);
		expect(isWorkspaceManager("member")).toBe(false);
		expect(isWorkspaceManager(null)).toBe(false);
	});
});
