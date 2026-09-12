import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Annotation } from "../../stores/editor-store.svelte";
import { expandTextAnnotations } from "./rasterize-text";

type Item = Pick<Annotation, "kind">;

function text(content: string): Item {
	return {
		kind: {
			kind: "text",
			x: 0,
			y: 0,
			w: 0.5,
			h: 0.1,
			content,
			fontFamily: "Inter",
			fontSize: 0.08,
			fontWeight: 400,
			color: "#ffffff",
			align: "center",
			lineHeight: 1.2,
		},
	};
}

const rect: Item = { kind: { kind: "rect", x: 0, y: 0, w: 0.2, h: 0.2, radius: 0 } };

/** No 2d context, which is exactly the failure the reporting exists for. */
function stubCanvas(context: unknown) {
	vi.stubGlobal("document", {
		fonts: { ready: Promise.resolve() },
		createElement: () => ({
			width: 0,
			height: 0,
			getContext: () => context,
			toDataURL: () => "data:image/png;base64,AAA",
		}),
	});
}

const workingContext = {
	font: "",
	fillStyle: "",
	textBaseline: "",
	textAlign: "",
	clearRect: () => undefined,
	fillText: () => undefined,
	measureText: (s: string) => ({ width: s.length }),
};

describe("expandTextAnnotations", () => {
	beforeEach(() => vi.unstubAllGlobals());

	it("names every annotation it had to drop instead of losing it silently", async () => {
		stubCanvas(null);
		const onDropped = vi.fn();

		const out = await expandTextAnnotations(
			[text("Ship it"), rect, text("Second")],
			100,
			100,
			onDropped,
		);

		expect(onDropped).toHaveBeenCalledTimes(1);
		expect(onDropped).toHaveBeenCalledWith(["Ship it", "Second"]);
		expect(out).toEqual([rect]);
	});

	it("says nothing when every annotation rendered", async () => {
		stubCanvas(workingContext);
		const onDropped = vi.fn();

		const out = await expandTextAnnotations([text("Ship it")], 100, 100, onDropped);

		expect(onDropped).not.toHaveBeenCalled();
		expect(out[0].kind.kind).toBe("image");
	});
});
