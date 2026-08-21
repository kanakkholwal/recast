/**
 * Optional hooks the host app can own but the editor must not require: product
 * analytics, the app-wide shortcut registry, and the export queue's rendering
 * flag. Each has a no-op default, so a host that installs nothing still gets a
 * working editor — the events go nowhere and the shortcuts stay unbound.
 */

export interface EditorAnalytics {
	capture(event: string, props?: Record<string, unknown>): void;
}

export interface ShortcutHost {
	/** Human-readable chord for an action id (e.g. "⌘S"), or "" when unbound. */
	chordLabel(id: string): string;
	/** Bind handlers for this editor's lifetime. Returns an unregister fn. */
	registerShortcutHandlers(handlers: Record<string, () => void>): () => void;
}

/** Workers the editor needs. The HOST APP owns every `new Worker(...)` call:
 *  a `new URL(…, import.meta.url)` inside this package resolves outside the
 *  app root, which then only fails in dev — see
 *  `packages/media/test/worker-resolution.test.ts`. */
export type EditorWorkerName = "mediabunny" | "render" | "filmstrip" | "smoothing" | "exportRender";

export interface WorkerHost {
	create(name: EditorWorkerName): Worker;
}

export interface ExportActivityHost {
	/** True while a browser export is compositing. The preview pauses its own
	 *  decode/render while set, so the two don't fight over the GPU. */
	readonly renderingInBrowser: boolean;
}

interface HostHooks {
	analytics: EditorAnalytics;
	workers: WorkerHost;
	shortcuts: ShortcutHost;
	exportActivity: ExportActivityHost;
}

const noop: HostHooks = {
	analytics: { capture: () => {} },
	workers: {
		create: (name) => {
			// Loud on purpose: a silently-missing worker degrades to no decode.
			throw new Error(
				`No worker host installed: cannot create the "${name}" worker. ` +
					"Call setEditorHostHooks({ workers }) from the app.",
			);
		},
	},
	shortcuts: { chordLabel: () => "", registerShortcutHandlers: () => () => {} },
	exportActivity: { renderingInBrowser: false },
};

let hooks: HostHooks = noop;

/** Install real implementations. Returns a restore fn so tests don't leak. */
export function setEditorHostHooks(next: Partial<HostHooks>): () => void {
	const previous = hooks;
	hooks = { ...hooks, ...next };
	return () => {
		hooks = previous;
	};
}

export const analytics: EditorAnalytics = {
	capture: (event, props) => hooks.analytics.capture(event, props),
};

export const chordLabel = (id: string): string => hooks.shortcuts.chordLabel(id);

export const registerShortcutHandlers = (handlers: Record<string, () => void>): (() => void) =>
	hooks.shortcuts.registerShortcutHandlers(handlers);

export const exportActivity: ExportActivityHost = {
	get renderingInBrowser() {
		return hooks.exportActivity.renderingInBrowser;
	},
};

/** Spawn a host-owned worker. Throws if the host installed no worker hook. */
export function createEditorWorker(name: EditorWorkerName): Worker {
	return hooks.workers.create(name);
}
