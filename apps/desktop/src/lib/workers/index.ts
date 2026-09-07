import type { EditorWorkerName, WorkerHost } from "@recast/editor";

/** Every `new URL` is a literal so the bundler can statically emit each chunk. */
export const workerHost: WorkerHost = {
	create(name: EditorWorkerName): Worker {
		switch (name) {
			case "mediabunny":
				return new Worker(new URL("./mediabunny.worker", import.meta.url), { type: "module" });
			case "filmstrip":
				return new Worker(new URL("./filmstrip.worker", import.meta.url), { type: "module" });
			case "smoothing":
				return new Worker(new URL("./smoothing.worker", import.meta.url), { type: "module" });
			case "exportRender":
				return new Worker(new URL("./export-render.worker", import.meta.url), { type: "module" });
		}
	},
};
