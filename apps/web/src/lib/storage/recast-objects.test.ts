import { beforeEach, describe, expect, it, vi } from "vitest";

const deleteObject = vi.fn<(key: string) => Promise<void>>();
vi.mock("$lib/storage", () => ({ deleteObject: (k: string) => deleteObject(k) }));

const { deleteRecastObjects } = await import("./recast-objects");

describe("deleteRecastObjects", () => {
	beforeEach(() => {
		deleteObject.mockReset();
		deleteObject.mockResolvedValue(undefined);
		vi.spyOn(console, "error").mockImplementation(() => undefined);
	});

	it("removes every object the recast owns, video and poster", async () => {
		await deleteRecastObjects("r_1", ["videos/r_1.mp4", "posters/r_1.jpg"]);
		expect(deleteObject.mock.calls.map(([k]) => k)).toEqual(["videos/r_1.mp4", "posters/r_1.jpg"]);
	});

	it("leaves external URLs alone: they aren't ours to delete", async () => {
		await deleteRecastObjects("r_1", ["https://cdn.example.com/x.mp4", null, undefined]);
		expect(deleteObject).not.toHaveBeenCalled();
	});

	it("deletes a repeated key once", async () => {
		await deleteRecastObjects("r_1", ["videos/r_1.mp4", "videos/r_1.mp4"]);
		expect(deleteObject).toHaveBeenCalledTimes(1);
	});

	it("swallows a provider failure so the committed delete still stands", async () => {
		deleteObject.mockRejectedValueOnce(new Error("403 from provider"));
		await expect(
			deleteRecastObjects("r_1", ["videos/r_1.mp4", "posters/r_1.jpg"]),
		).resolves.toBeUndefined();
		expect(deleteObject).toHaveBeenCalledTimes(2);
	});
});
